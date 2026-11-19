use crate::{
    coordinator::{Engine, Input},
    logging::record,
    login::{Action, Backend, State},
    menu, power,
    preferences::{Config, Store, TEST},
    presentation,
    scheduler::{Pause, Periods, Settings},
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::Manager;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub settings: Settings,
    pub pause: Option<Pause>,
    pub sound_enabled: bool,
    pub login: State,
    pub login_requested: bool,
    pub login_busy: bool,
    pub login_error: Option<String>,
    pub saving: bool,
    pub save_error: Option<String>,
}
#[derive(Debug)]
enum Msg {
    Core(Input),
    ToggleSound,
    Power(bool, u64),
    Login(Action),
    Retry,
    Saved(u64, Result<(), String>),
    Observed(State, Option<String>),
}
#[derive(Clone)]
pub struct Channel {
    tx: Sender<Msg>,
    stopping: Arc<AtomicBool>,
    pub permit: Arc<AtomicU64>,
    pub visible_attempt: Arc<AtomicU64>,
    pub asleep: Arc<AtomicBool>,
    signals: Arc<Mutex<(u64, bool, u64)>>, // latest timestamp, physical state, last transition
}
impl Channel {
    fn new(tx: Sender<Msg>) -> Self {
        Self {
            tx,
            stopping: Arc::new(AtomicBool::new(false)),
            permit: Arc::new(AtomicU64::new(0)),
            visible_attempt: Arc::new(AtomicU64::new(0)),
            asleep: Arc::new(AtomicBool::new(false)),
            signals: Arc::new(Mutex::new((0, false, 0))),
        }
    }
    pub fn stop(&self) {
        if !self.stopping.swap(true, Ordering::AcqRel) {
            self.core(Input::Stop);
        }
    }
    pub fn core(&self, i: Input) {
        let _ = self.tx.send(Msg::Core(i));
    }
    pub fn power(&self, sleep: bool, ns: u64) {
        let Ok(mut state) = self.signals.lock() else {
            return;
        };
        if ns < state.0 {
            return;
        }
        state.0 = ns;
        if sleep == state.1 {
            return;
        }
        state.1 = sleep;
        state.2 = ns;
        if sleep {
            self.asleep.store(true, Ordering::Release);
            self.permit.store(0, Ordering::Release);
            self.visible_attempt.store(0, Ordering::Release);
        }
        let _ = self.tx.send(Msg::Power(sleep, ns));
    }
    pub fn can_show(&self, id: u64) -> bool {
        !self.asleep.load(Ordering::Acquire) && self.permit.load(Ordering::Acquire) == id
    }
}
pub struct Bridge {
    ch: Channel,
}
impl Bridge {
    pub fn stop(&self) {
        self.ch.stop();
    }
    pub fn is_stopping(&self) -> bool {
        self.ch.stopping.load(Ordering::Acquire)
    }
    pub fn send(&self, i: Input) {
        if !self.is_stopping() {
            self.ch.core(i);
        }
    }
    pub fn login(&self, a: Action) {
        let _ = self.ch.tx.send(Msg::Login(a));
    }
    pub fn toggle_sound(&self) {
        let _ = self.ch.tx.send(Msg::ToggleSound);
    }
    pub fn retry(&self) {
        let _ = self.ch.tx.send(Msg::Retry);
    }
}
fn receive<T>(
    rx: &Receiver<T>,
    deadline: Option<Duration>,
    now: Duration,
    wake: impl FnOnce() -> T,
) -> Option<T> {
    match deadline {
        Some(t) => match rx.recv_timeout(t.saturating_sub(now)) {
            Ok(i) => Some(i),
            Err(RecvTimeoutError::Timeout) => Some(wake()),
            Err(RecvTimeoutError::Disconnected) => None,
        },
        None => rx.recv().ok(),
    }
}
enum Disk {
    Save(u64, Config),
    Login(Action),
    First(Config),
    Drain(Sender<()>),
}
fn first_action(fresh: bool, saved: bool, test: bool) -> Action {
    if fresh && saved && !test {
        Action::Set(true)
    } else {
        Action::Query
    }
}
fn storage_worker(store: Store, ch: Channel, previous: State) -> Sender<Disk> {
    let mut backend = Backend::new(previous);
    storage_jobs(store, ch, move |a| backend.run(a))
}
fn storage_jobs(
    store: Store,
    ch: Channel,
    mut login: impl FnMut(Action) -> (State, Option<String>) + Send + 'static,
) -> Sender<Disk> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for job in rx {
            let action = match job {
                Disk::Save(rev, cfg) => {
                    let result = store.save(&cfg).map_err(|e| e.to_string());
                    let _ = ch.tx.send(Msg::Saved(rev, result));
                    continue;
                }
                Disk::First(cfg) => {
                    let result = store.save(&cfg).map_err(|e| e.to_string());
                    let action = first_action(true, result.is_ok(), TEST);
                    let _ = ch.tx.send(Msg::Saved(0, result));
                    action
                }
                Disk::Login(a) => a,
                Disk::Drain(done) => {
                    let _ = done.send(());
                    break;
                }
            };
            let (state, err) = login(action);
            let _ = ch.tx.send(Msg::Observed(state, err));
        }
    });
    tx
}
fn update_cfg(cfg: &mut Config, engine: &Engine) {
    let s = engine.settings();
    cfg.stretch_enabled = s.stretch_enabled;
    cfg.water_enabled = s.water_enabled;
    cfg.stretch_minutes = s.stretch_interval.minutes();
    cfg.water_minutes = s.water_interval.minutes();
    cfg.indefinite = engine.pause() == Some(Pause::Indefinite);
}
pub fn start(app: tauri::AppHandle, store: Store) -> tauri::Result<()> {
    let (cfg, fresh, problem) = match store.load() {
        Ok(Some(c)) => (c, false, None),
        Ok(None) => (Config::default(), true, None),
        Err(e) => (Config::default(), false, Some(e.to_string())),
    };
    let periods = if TEST {
        Periods {
            thirty: Duration::from_secs(30),
            sixty: Duration::from_secs(60),
        }
    } else {
        Periods::normal()
    };
    let mut engine = Engine::restore(periods, cfg.settings(), cfg.indefinite);
    let mut view = View {
        settings: engine.settings(),
        pause: engine.pause(),
        sound_enabled: cfg.sound_enabled,
        login: State::Unknown,
        login_requested: cfg.login_requested,
        login_busy: true,
        login_error: None,
        saving: fresh,
        save_error: problem,
    };
    let (tx, rx) = mpsc::channel();
    let ch = Channel::new(tx);
    let stopping = ch.stopping.clone();
    app.manage(Bridge { ch: ch.clone() });
    menu::install(&app, &view)?;
    let mut clock = power::Clock::new(power::raw());
    power::install(ch.clone());
    let previous = if TEST {
        if cfg.login_requested {
            State::Enabled
        } else {
            State::Disabled
        }
    } else {
        cfg.login_observed
    };
    let disk = storage_worker(store, ch.clone(), previous);
    let _ = disk.send(if fresh {
        Disk::First(cfg.clone())
    } else {
        Disk::Login(first_action(false, false, TEST))
    });
    std::thread::spawn(move || {
        let mut cfg = cfg;
        let mut revision = 0;
        let mut previous_step = Duration::ZERO;
        while let Some(msg) = receive(&rx, engine.next_wake(), clock.now(power::raw()), || {
            Msg::Core(Input::Wake)
        }) {
            let before = view.clone();
            let old = cfg.clone();
            let mut save = false;
            let (input, now) = match msg {
                Msg::Power(sleep, stamp) => {
                    if !clock.transition(sleep, stamp) {
                        continue;
                    }
                    if !sleep {
                        if let Ok(state) = ch.signals.lock() {
                            if !state.1 && state.2 == stamp {
                                ch.asleep.store(false, Ordering::Release);
                            }
                        }
                    }
                    (
                        if sleep { Input::Sleep } else { Input::Awaken },
                        clock.now(stamp),
                    )
                }
                Msg::Core(Input::Stop) => {
                    engine.step(clock.now(power::raw()), Input::Stop);
                    presentation::shutdown(&app);
                    let (done, ack) = mpsc::channel();
                    let _ = disk.send(Disk::Drain(done));
                    if ack.recv_timeout(Duration::from_millis(800)).is_err() {
                        record("Salida: no se confirmó el guardado dentro de 800 ms.");
                    }
                    app.exit(0);
                    break;
                }
                Msg::Core(i) => (i, clock.now(power::raw())),
                other => {
                    match other {
                        Msg::Login(action) if !view.login_busy => {
                            if let Action::Set(on) = action {
                                cfg.login_requested = on;
                            }
                            view.login_busy = true;
                            view.login_error = None;
                            let _ = disk.send(Disk::Login(action));
                        }
                        Msg::Saved(rev, result) if rev == revision => {
                            view.saving = false;
                            view.save_error = result.err();
                        }
                        Msg::Observed(observed, error) => {
                            view.login_busy = false;
                            view.login = observed;
                            cfg.login_observed = observed;
                            view.login_error = error;
                        }
                        Msg::Retry => save = true,
                        Msg::ToggleSound => cfg.sound_enabled = !cfg.sound_enabled,
                        _ => {}
                    };
                    (Input::Wake, clock.now(power::raw()))
                }
            };
            let now = now.max(previous_step);
            previous_step = now;
            // macOS toggles a check item on click even when selecting the same interval.
            let redraw = matches!(input, Input::Interval(_, _));
            let effects = engine.step(now, input);
            update_cfg(&mut cfg, &engine);
            if save || cfg != old {
                revision += 1;
                view.saving = true;
                if disk.send(Disk::Save(revision, cfg.clone())).is_err() {
                    view.saving = false;
                    view.save_error = Some("Guardado no disponible".into());
                }
            }
            view.pause = engine.pause();
            view.sound_enabled = cfg.sound_enabled;
            view.settings = engine.settings();
            view.login_requested = cfg.login_requested;
            if redraw || view != before {
                menu::synchronize(&app, &view);
            }
            for effect in effects {
                presentation::execute(&app, effect, &ch, &stopping, cfg.sound_enabled);
            }
        }
        record("Motor detenido.");
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_interval_choices_save_reload_and_restore_independently() {
        use crate::preferences::Opened;
        use crate::scheduler::{Kind, ReminderInterval};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".development")
            .join(format!("interval-store-test-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        let Opened::Owner(store, guard) = Store::acquire(&path, "normal").unwrap() else {
            panic!("expected isolated preferences");
        };
        let choices = [
            ReminderInterval::Thirty,
            ReminderInterval::Sixty,
            ReminderInterval::TwoHours,
        ];
        let periods = Periods {
            thirty: Duration::from_secs(30),
            sixty: Duration::from_secs(60),
        };
        for stretch in choices {
            for water in choices {
                let mut cfg = Config {
                    sound_enabled: false,
                    login_requested: false,
                    indefinite: true,
                    ..Config::default()
                };
                let mut engine = Engine::restore(periods, cfg.settings(), cfg.indefinite);
                engine.step(Duration::ZERO, Input::Interval(Kind::Stretch, stretch));
                engine.step(Duration::ZERO, Input::Interval(Kind::Water, water));
                update_cfg(&mut cfg, &engine);
                assert_eq!(
                    (cfg.stretch_minutes, cfg.water_minutes),
                    (stretch.minutes(), water.minutes())
                );
                store.save(&cfg).unwrap();
                let loaded = store.load().unwrap().unwrap();
                assert_eq!(loaded, cfg);
                let mut restored = Engine::restore(periods, loaded.settings(), loaded.indefinite);
                assert_eq!(restored.settings().stretch_interval, stretch);
                assert_eq!(restored.settings().water_interval, water);
                assert_eq!(restored.next_wake(), None);
                restored.step(Duration::from_secs(5), Input::Resume);
                assert_eq!(
                    restored.next_wake(),
                    Some(Duration::from_secs(
                        5 + u64::from(stretch.minutes().min(water.minutes()))
                    ))
                );
                assert!(!loaded.sound_enabled);
            }
        }
        drop(guard);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn no_deadline_receives_manual_and_stop_instead_of_ending_worker() {
        let (tx, rx) = mpsc::channel();
        tx.send(Input::Manual(crate::scheduler::Kind::Water))
            .unwrap();
        assert!(matches!(
            receive(&rx, None, Duration::ZERO, || Input::Wake),
            Some(Input::Manual(_))
        ));
        tx.send(Input::Stop).unwrap();
        assert!(matches!(
            receive(&rx, None, Duration::ZERO, || Input::Wake),
            Some(Input::Stop)
        ));
    }
    #[test]
    fn denied_or_pending_login_is_reported_and_preference_saves_do_not_retry_it() {
        use crate::preferences::Opened;
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".development")
            .join(format!("login-io-test-{}", std::process::id()));
        std::fs::create_dir(&p).unwrap();
        let Opened::Owner(store, guard) = Store::acquire(&p, "normal").unwrap() else {
            panic!()
        };
        let (tx, rx) = mpsc::channel();
        let ch = Channel::new(tx);
        let verify = store.clone();
        let disk = storage_jobs(store, ch, move |action| {
            assert!(verify.load().unwrap().is_some());
            match action {
                Action::Query | Action::Set(true) => (State::NeedsApproval, None),
                Action::Set(false) => (State::Enabled, Some("Denegado".into())),
            }
        });
        disk.send(Disk::First(Config::default())).unwrap();
        assert!(matches!(rx.recv().unwrap(), Msg::Saved(0, Ok(()))));
        assert!(matches!(
            rx.recv().unwrap(),
            Msg::Observed(State::NeedsApproval, None)
        ));
        disk.send(Disk::Login(Action::Set(false))).unwrap();
        assert!(matches!(
            rx.recv().unwrap(),
            Msg::Observed(State::Enabled, Some(_))
        ));
        disk.send(Disk::Save(
            1,
            Config {
                indefinite: true,
                sound_enabled: false,
                ..Config::default()
            },
        ))
        .unwrap();
        assert!(matches!(rx.recv().unwrap(), Msg::Saved(1, Ok(()))));
        let (done, ack) = mpsc::channel();
        disk.send(Disk::Drain(done)).unwrap();
        ack.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(rx.try_recv().is_err());
        drop(guard);
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn due_deadline_returns_wake_and_disconnected_channel_ends_wait() {
        let (tx, rx) = mpsc::channel();
        assert!(matches!(
            receive(&rx, Some(Duration::ZERO), Duration::ZERO, || Input::Wake),
            Some(Input::Wake)
        ));
        drop(tx);
        assert!(receive(&rx, None, Duration::ZERO, || Input::Wake).is_none());
    }
    #[test]
    fn registration_requires_clean_first_save_never_retries_existing_or_test() {
        assert_eq!(first_action(true, true, false), Action::Set(true));
        for (f, s, t) in [
            (false, true, false),
            (true, false, false),
            (true, true, true),
        ] {
            assert_eq!(first_action(f, s, t), Action::Query);
        }
    }
    #[cfg(feature = "test-profile")]
    #[test]
    fn test_login_simulates_both_states() {
        let mut b = Backend::new(State::Disabled);
        assert_eq!(b.run(Action::Set(true)), (State::Enabled, None));
        assert_eq!(b.run(Action::Set(false)), (State::Disabled, None));
    }
}
