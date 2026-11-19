use crate::scheduler::Kind;
use crate::{
    audio,
    coordinator::{Effect, Input, DISPLAY_TIME},
    logging::record,
    scheduler::Request,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{LogicalSize, Manager};
extern "C" {
    fn sw_reminder_prepare(
        window: *mut std::ffi::c_void,
        id: u64,
        stretch: i32,
        test_profile: i32,
    ) -> i32;
    fn sw_reminder_begin(id: u64, remaining_ms: u64) -> i32;
    fn sw_reminder_stop(draws: *mut i32, errors: *mut i32) -> i32;
    fn sw_reminder_position_at_cursor(window: *mut std::ffi::c_void, margin: f64) -> i32;
}
const WINDOW: &str = "reminder";
const MARGIN: f64 = 16.0;
type Failure = Box<dyn std::error::Error>;

fn window_size() -> LogicalSize<f64> {
    LogicalSize::new(290.0, 298.0)
}

fn prepare(app: &tauri::AppHandle, request: &Request) -> Result<(), Failure> {
    let window = if let Some(window) = app.get_window(WINDOW) {
        window.hide()?;
        window
    } else {
        let size = window_size();
        let window = tauri::WindowBuilder::new(app, WINDOW)
            .title(
                app.config()
                    .product_name
                    .as_deref()
                    .unwrap_or("Stretch & Water"),
            )
            .inner_size(size.width, size.height)
            .visible(false)
            .focused(false)
            .focusable(false)
            .transparent(true)
            .decorations(false)
            .shadow(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .build()?;
        if let Err(e) = window.set_ignore_cursor_events(true) {
            let _ = window.destroy();
            return Err(e.into());
        }
        window
    };
    let prepared = unsafe {
        sw_reminder_prepare(
            window.ns_window()?,
            request.id,
            (request.kind == Kind::Stretch) as i32,
            i32::from(crate::preferences::TEST),
        )
    };
    if prepared == 2 {
        record("Secuencia no disponible: se muestra la figura de reserva.");
    } else if prepared == 0 {
        return Err("No se pudo preparar la primera pose".into());
    }
    Ok(())
}
fn show(app: &tauri::AppHandle) -> Result<(), Failure> {
    let found = app.get_window(WINDOW);
    let window = found.ok_or("Ventana no disponible")?;
    window.set_ignore_cursor_events(true)?;
    if unsafe { sw_reminder_position_at_cursor(window.ns_window()?, MARGIN) } == 0 {
        return Err("No se pudo ubicar el aviso en una pantalla disponible".into());
    }
    window.show()?;
    Ok(())
}
fn hide(app: &tauri::AppHandle) -> Result<(), Failure> {
    if let Some(window) = app.get_window(WINDOW) {
        let hidden = window.hide();
        let (mut draws, mut errors) = (0, 0);
        let last = unsafe { sw_reminder_stop(&mut draws, &mut errors) };
        if last >= 0 {
            record(&format!(
                "Fotograma final {last}; poses dibujadas {draws}; errores de lectura {errors}"
            ));
        }
        hidden?;
    }
    Ok(())
}
pub fn shutdown(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        audio::stop();
        if let Err(e) = hide(&handle) {
            record(&format!("Ocultar al salir: {e}"));
        }
    });
}

fn start_media(
    sound_enabled: bool,
    reveal: impl FnOnce() -> Result<(), Failure>,
    play: impl FnOnce() -> bool,
) -> Result<(), Failure> {
    reveal()?;
    if sound_enabled && !play() {
        record("No se pudo reproducir cling.wav.");
    }
    Ok(())
}

fn reveal(id: u64, duration_ms: u128) -> Result<(), Failure> {
    if unsafe { sw_reminder_begin(id, duration_ms as u64) } == 0 {
        return Err("El reproductor no pudo iniciar esta ronda".into());
    }
    Ok(())
}
pub fn execute(
    app: &tauri::AppHandle,
    effect: Effect,
    tx: &crate::runtime::Channel,
    stopping: &Arc<AtomicBool>,
    sound_enabled: bool,
) {
    record(&format!("Efecto {effect:?}"));
    match &effect {
        Effect::Prepare(r) => tx.permit.store(r.id, Ordering::Release),
        Effect::CancelShow(id) => {
            let _ = tx
                .permit
                .compare_exchange(*id, 0, Ordering::AcqRel, Ordering::Acquire);
            return;
        }
        Effect::Hide(_) => {
            tx.permit.store(0, Ordering::Release);
            tx.visible_attempt.store(0, Ordering::Release);
        }
        Effect::Begin(_) | Effect::Show(_) => {}
    }
    let dispatch_app = app.clone();
    let sender = tx.clone();
    let canceled = Arc::clone(stopping);
    let scheduled = std::time::Instant::now();
    let failure = match &effect {
        Effect::CancelShow(_) => unreachable!(),
        Effect::Prepare(r) => Some(Input::Prepared {
            id: r.id,
            ok: false,
        }),
        Effect::Show(id) => Some(Input::Shown { id: *id, ok: false }),
        Effect::Hide(id) => Some(Input::Hidden { id: *id, ok: false }),
        Effect::Begin(_) => None,
    };
    if let Err(error) = app.run_on_main_thread(move || {
        if canceled.load(Ordering::Acquire) {
            return;
        }
        match effect {
            Effect::CancelShow(_) => unreachable!(),
            Effect::Prepare(request) => {
                if !sender.can_show(request.id) {
                    return;
                }
                if let Err(error) = prepare(&dispatch_app, &request) {
                    record(&format!("Preparación fallida id={}: {error}", request.id));
                    sender.core(Input::Prepared {
                        id: request.id,
                        ok: false,
                    });
                } else {
                    sender.core(Input::Prepared {
                        id: request.id,
                        ok: true,
                    });
                }
            }
            Effect::Show(id) => {
                if !sender.can_show(id) {
                    sender.core(Input::Shown { id, ok: false });
                    return;
                }
                let result = show(&dispatch_app);
                if let Err(error) = &result {
                    record(&format!("Mostrar falló id={id}: {error}"));
                } else {
                    sender.visible_attempt.store(id, Ordering::Release);
                }
                sender.core(Input::Shown {
                    id,
                    ok: result.is_ok(),
                });
            }
            Effect::Begin(id) => {
                let remaining = DISPLAY_TIME.saturating_sub(scheduled.elapsed()).as_millis();
                if sender.asleep.load(Ordering::Acquire)
                    || sender.visible_attempt.load(Ordering::Acquire) != id
                    || remaining == 0
                {
                    return;
                }
                if let Err(e) = start_media(sound_enabled, || reveal(id, remaining), audio::play) {
                    record(&format!("Inicio de la presentación {id}: {e}"));
                }
            }
            Effect::Hide(id) => {
                audio::stop();
                let result = hide(&dispatch_app);
                if let Err(error) = &result {
                    record(&format!("Ocultar falló id={id}; cola bloqueada: {error}"));
                }
                sender.core(Input::Hidden {
                    id,
                    ok: result.is_ok(),
                });
            }
        }
    }) {
        record(&format!("Error de despacho nativo: {error}"));
        if let Some(input) = failure {
            tx.core(input);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::start_media;
    #[test]
    fn simultaneous_automatics_only_sound_when_each_one_is_shown() {
        use crate::coordinator::{Effect, Engine, Input};
        use crate::scheduler::{Kind, Periods};
        use std::time::Duration;
        let s = Duration::from_secs;
        let mut engine = Engine::new(Periods {
            thirty: s(30),
            sixty: s(60),
        });
        engine.step(
            s(0),
            Input::Interval(Kind::Stretch, crate::scheduler::ReminderInterval::Sixty),
        );
        let mut fx = engine.step(s(60), Input::Wake);
        let mut heard = Vec::new();
        for (start, kind) in [(60, Kind::Stretch), (77, Kind::Water)] {
            let [Effect::Prepare(request)] = &fx[..] else {
                panic!("expected preparation")
            };
            assert_eq!(request.kind, kind);
            let id = request.id;
            assert_eq!(
                engine.step(s(start), Input::Prepared { id, ok: true }),
                [Effect::Show(id)]
            );
            assert_eq!(
                engine.step(s(start), Input::Shown { id, ok: true }),
                [Effect::Begin(id)]
            );
            // Mute while water is in the queue. A failed playback may not delay it.
            start_media(
                start == 60,
                || Ok(()),
                || {
                    heard.push(kind);
                    false
                },
            )
            .unwrap();
            assert_eq!(engine.step(s(start + 12), Input::Wake), [Effect::Hide(id)]);
            assert!(engine
                .step(s(start + 12), Input::Hidden { id, ok: true })
                .is_empty());
            assert!(engine.step(s(start + 16), Input::Wake).is_empty());
            fx = engine.step(s(start + 17), Input::Wake);
        }
        assert_eq!(heard, [Kind::Stretch]);
    }
    #[test]
    fn muting_or_visual_failure_never_plays_and_audio_failure_finishes_normally() {
        let calls = std::cell::Cell::new(0);
        let failing_player = || {
            calls.set(calls.get() + 1);
            false
        };
        start_media(false, || Ok(()), failing_player).unwrap();
        assert!(start_media(true, || Err("no visible".into()), failing_player).is_err());
        assert_eq!(calls.get(), 0);
        start_media(true, || Ok(()), failing_player).unwrap();
        assert_eq!(calls.get(), 1);
    }
}
