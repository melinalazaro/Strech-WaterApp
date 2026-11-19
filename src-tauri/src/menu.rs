use crate::{
    coordinator::Input,
    logging::record,
    login::{Action, State},
    preferences::TEST,
    runtime::{Bridge, View},
    scheduler::{Kind, Pause, ReminderInterval},
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    Manager, Wry,
};
struct Controls {
    icon: tauri::tray::TrayIcon,
    menu: Menu<Wry>,
    stretch: Submenu<Wry>,
    water: Submenu<Wry>,
    s_on: CheckMenuItem<Wry>,
    w_on: CheckMenuItem<Wry>,
    stretch_thirty: CheckMenuItem<Wry>,
    stretch_sixty: CheckMenuItem<Wry>,
    stretch_two_hours: CheckMenuItem<Wry>,
    water_thirty: CheckMenuItem<Wry>,
    water_sixty: CheckMenuItem<Wry>,
    water_two_hours: CheckMenuItem<Wry>,
    sound: CheckMenuItem<Wry>,
    state: MenuItem<Wry>,
    resume: MenuItem<Wry>,
    saved: MenuItem<Wry>,
    retry: MenuItem<Wry>,
    retry_present: AtomicBool,
    login: CheckMenuItem<Wry>,
    login_on: AtomicBool,
    login_info: MenuItem<Wry>,
    disable: MenuItem<Wry>,
    query: MenuItem<Wry>,
}
fn title(s: &str, on: bool) -> String {
    if on {
        format!("✓ {s}")
    } else {
        s.to_owned()
    }
}
fn unit() -> &'static str {
    if TEST {
        "segundos"
    } else {
        "minutos"
    }
}
fn interval_label(interval: ReminderInterval) -> String {
    if !TEST && interval == ReminderInterval::TwoHours {
        "Cada 2 horas".into()
    } else {
        format!("Cada {} {}", interval.minutes(), unit())
    }
}
fn status(v: &View) -> String {
    let text = match v.pause {
        None => "Activa".into(),
        Some(Pause::Indefinite) => "Pausada indefinidamente".into(),
        Some(Pause::Thirty) => format!("Pausada 30 {}", unit()),
        Some(Pause::Sixty) => format!("Pausada 60 {}", unit()),
    };
    if TEST {
        format!("PRUEBA · {text}")
    } else {
        text
    }
}
fn tray(v: &View) -> String {
    format!(
        "{}{}",
        if TEST { "SW·T" } else { "SW" },
        if v.pause.is_some() { " · pausa" } else { "" }
    )
}
impl Controls {
    fn render(&self, v: &View) -> tauri::Result<()> {
        let s = v.settings;
        self.icon.set_title(Some(tray(v)))?;
        self.state.set_text(status(v))?;
        self.stretch
            .set_text(title("Estiramientos", s.stretch_enabled))?;
        self.water.set_text(title("Tomar agua", s.water_enabled))?;
        self.sound.set_checked(v.sound_enabled)?;
        self.s_on.set_checked(s.stretch_enabled)?;
        self.w_on.set_checked(s.water_enabled)?;
        self.stretch_thirty
            .set_checked(s.stretch_interval == ReminderInterval::Thirty)?;
        self.stretch_sixty
            .set_checked(s.stretch_interval == ReminderInterval::Sixty)?;
        self.stretch_two_hours
            .set_checked(s.stretch_interval == ReminderInterval::TwoHours)?;
        self.water_thirty
            .set_checked(s.water_interval == ReminderInterval::Thirty)?;
        self.water_sixty
            .set_checked(s.water_interval == ReminderInterval::Sixty)?;
        self.water_two_hours
            .set_checked(s.water_interval == ReminderInterval::TwoHours)?;
        self.resume.set_enabled(v.pause.is_some())?;
        let on = v.login == State::Enabled;
        self.login_on.store(on, Ordering::Release);
        self.login.set_checked(on)?;
        self.login.set_enabled(!v.login_busy)?;
        self.query.set_enabled(!v.login_busy)?;
        self.disable
            .set_enabled(!v.login_busy && !on && v.login_requested)?;
        self.login_info.set_text(if v.login_busy {
            "Comprobando…".into()
        } else if let Some(e) = &v.login_error {
            format!("{} · {e}", v.login.label())
        } else {
            v.login.label().to_owned()
        })?;
        self.saved.set_text(if v.saving {
            "Guardando preferencias…".into()
        } else if let Some(e) = &v.save_error {
            format!("Cambios sin guardar · {e}")
        } else {
            "Preferencias guardadas".into()
        })?;
        let need = v.save_error.is_some();
        if need != self.retry_present.load(Ordering::Acquire) {
            if need {
                self.menu.append(&self.retry)?;
            } else {
                self.menu.remove(&self.retry)?;
            }
            self.retry_present.store(need, Ordering::Release);
        }
        Ok(())
    }
}
pub fn install(app: &tauri::AppHandle, v: &View) -> tauri::Result<()> {
    let item = |id: &str, text: &str, on: bool| MenuItem::with_id(app, id, text, on, None::<&str>);
    let check = |id: &str, text: &str, on: bool| {
        CheckMenuItem::with_id(app, id, text, true, on, None::<&str>)
    };
    let sep = || PredefinedMenuItem::separator(app);
    let s = v.settings;
    let s_on = check("stretch-enabled", "Activado", s.stretch_enabled)?;
    let w_on = check("water-enabled", "Activado", s.water_enabled)?;
    let stretch_thirty = check(
        "stretch-30",
        &interval_label(ReminderInterval::Thirty),
        s.stretch_interval == ReminderInterval::Thirty,
    )?;
    let stretch_sixty = check(
        "stretch-60",
        &interval_label(ReminderInterval::Sixty),
        s.stretch_interval == ReminderInterval::Sixty,
    )?;
    let stretch_two_hours = check(
        "stretch-120",
        &interval_label(ReminderInterval::TwoHours),
        s.stretch_interval == ReminderInterval::TwoHours,
    )?;
    let water_thirty = check(
        "water-30",
        &interval_label(ReminderInterval::Thirty),
        s.water_interval == ReminderInterval::Thirty,
    )?;
    let water_sixty = check(
        "water-60",
        &interval_label(ReminderInterval::Sixty),
        s.water_interval == ReminderInterval::Sixty,
    )?;
    let water_two_hours = check(
        "water-120",
        &interval_label(ReminderInterval::TwoHours),
        s.water_interval == ReminderInterval::TwoHours,
    )?;
    let stretch = Submenu::with_items(
        app,
        title("Estiramientos", s.stretch_enabled),
        true,
        &[
            &s_on,
            &sep()?,
            &stretch_thirty,
            &stretch_sixty,
            &stretch_two_hours,
            &sep()?,
            &item("stretch", "Estirarme ahora", true)?,
        ],
    )?;
    let water = Submenu::with_items(
        app,
        title("Tomar agua", s.water_enabled),
        true,
        &[
            &w_on,
            &sep()?,
            &water_thirty,
            &water_sixty,
            &water_two_hours,
            &sep()?,
            &item("water", "Tomar agua ahora", true)?,
        ],
    )?;
    let sound = check("sound", "Sonido", v.sound_enabled)?;
    let state = item("state", &status(v), false)?;
    let resume = item("resume", "Reanudar", v.pause.is_some())?;
    let saved = item("saved", "Preferencias", false)?;
    let retry = item("retry-save", "Reintentar guardar cambios", true)?;
    let login = check("login", "Iniciar al iniciar sesión", false)?;
    let login_info = item("login-info", "Comprobando…", false)?;
    let disable = item("login-off", "Cancelar solicitud de inicio", false)?;
    let query = item("login-query", "Actualizar estado", true)?;
    let login_menu = Submenu::with_items(
        app,
        if TEST {
            "Inicio de sesión (simulado)"
        } else {
            "Inicio de sesión"
        },
        true,
        &[&login, &login_info, &disable, &query],
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &state,
            &stretch,
            &water,
            &sound,
            &sep()?,
            &item("pause30", &format!("Pausar 30 {}", unit()), true)?,
            &item(
                "pause60",
                if TEST {
                    "Pausar 60 segundos"
                } else {
                    "Pausar 1 hora"
                },
                true,
            )?,
            &item("pause-inf", "Pausar indefinidamente", true)?,
            &resume,
            &sep()?,
            &login_menu,
            &saved,
            &sep()?,
            &item("quit", "Salir", true)?,
        ],
    )?;
    let icon = TrayIconBuilder::with_id("stretch-water")
        .icon(tray_image())
        .icon_as_template(true)
        .title(tray(v))
        .tooltip("Stretch & Water")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let b = app.state::<Bridge>();
            match event.id.as_ref() {
                "stretch" => b.send(Input::Manual(Kind::Stretch)),
                "sound" => b.toggle_sound(),
                "water" => b.send(Input::Manual(Kind::Water)),
                "stretch-enabled" => b.send(Input::ToggleAutomatic(Kind::Stretch)),
                "water-enabled" => b.send(Input::ToggleAutomatic(Kind::Water)),
                "stretch-30" => b.send(Input::Interval(Kind::Stretch, ReminderInterval::Thirty)),
                "stretch-60" => b.send(Input::Interval(Kind::Stretch, ReminderInterval::Sixty)),
                "stretch-120" => b.send(Input::Interval(Kind::Stretch, ReminderInterval::TwoHours)),
                "water-30" => b.send(Input::Interval(Kind::Water, ReminderInterval::Thirty)),
                "water-60" => b.send(Input::Interval(Kind::Water, ReminderInterval::Sixty)),
                "water-120" => b.send(Input::Interval(Kind::Water, ReminderInterval::TwoHours)),
                "pause30" => b.send(Input::Pause(Pause::Thirty)),
                "pause60" => b.send(Input::Pause(Pause::Sixty)),
                "pause-inf" => b.send(Input::Pause(Pause::Indefinite)),
                "resume" => b.send(Input::Resume),
                "retry-save" => b.retry(),
                "login-query" => b.login(Action::Query),
                "login-off" => b.login(Action::Set(false)),
                "login" => {
                    if let Some(c) = app.try_state::<Controls>() {
                        b.login(Action::Set(!c.login_on.load(Ordering::Acquire)));
                    }
                }
                "quit" => b.stop(),
                _ => {}
            }
        })
        .build(app)?;
    icon.set_visible(true)?;
    let controls = Controls {
        icon,
        menu,
        stretch,
        water,
        s_on,
        w_on,
        stretch_thirty,
        stretch_sixty,
        stretch_two_hours,
        water_thirty,
        water_sixty,
        water_two_hours,
        state,
        sound,
        resume,
        saved,
        retry,
        retry_present: AtomicBool::new(false),
        login,
        login_info,
        disable,
        query,
        login_on: AtomicBool::new(false),
    };
    controls.render(v)?;
    app.manage(controls);
    Ok(())
}
pub fn synchronize(app: &tauri::AppHandle, v: &View) {
    let handle = app.clone();
    let v = v.clone();
    if let Err(e) = app.run_on_main_thread(move || {
        if let Some(c) = handle.try_state::<Controls>() {
            if let Err(err) = c.render(&v) {
                record(&format!("Error de menú: {err}"));
            }
        }
    }) {
        record(&format!("Error de menú: {e}"));
    }
}
fn tray_image() -> Image<'static> {
    const LOTUS: &[u8; 72 * 72 * 4] = include_bytes!("../icons/lotus-tray.rgba");
    Image::new(LOTUS, 72, 72)
}
