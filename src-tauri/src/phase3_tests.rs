//! Acceptance boundaries with an injected awake clock and presenter.
use super::{
    coordinator::{Effect, Engine, Input},
    power::Clock,
    scheduler::{Kind, Origin, Pause, Periods, ReminderInterval, Scheduler, Settings},
};
use std::time::Duration;
fn t(n: u64) -> Duration {
    Duration::from_secs(n)
}
fn periods() -> Periods {
    Periods {
        thirty: t(30),
        sixty: t(60),
    }
}
fn s() -> Scheduler {
    Scheduler::new(periods())
}
fn new() -> Engine {
    Engine::new(periods())
}
fn prepared(fx: Vec<Effect>, kind: Kind) -> u64 {
    match &fx[..] {
        [Effect::Prepare(r)] if r.kind == kind => r.id,
        _ => panic!("Unexpected effects: {fx:?}"),
    }
}
fn show(e: &mut Engine, at: u64, id: u64) {
    assert_eq!(
        e.step(t(at), Input::Prepared { id, ok: true }),
        [Effect::Show(id)]
    );
    assert_eq!(
        e.step(t(at), Input::Shown { id, ok: true }),
        [Effect::Begin(id)]
    );
}
#[test]
fn automatic_resume_uses_exact_expiry_even_if_woken_late() {
    let mut c = s();
    c.set_pause(t(10), Pause::Thirty);
    assert_eq!(c.next_due(), Some(t(40)));
    c.collect(t(41));
    assert_eq!(c.pause, None);
    assert_eq!(c.next_due(), Some(t(60)));
    assert!(c.pop().is_none());
    c.collect(t(90));
    let events: Vec<_> = std::iter::from_fn(|| c.pop())
        .map(|r| (r.kind, r.at))
        .collect();
    assert_eq!(
        events,
        [
            (Kind::Stretch, t(60)),
            (Kind::Stretch, t(90)),
            (Kind::Water, t(90))
        ]
    );
}
#[test]
fn replacing_pause_does_not_recapture_remaining_and_resume_is_idempotent() {
    let mut c = s();
    c.set_pause(t(10), Pause::Thirty);
    c.set_pause(t(15), Pause::Sixty);
    assert_eq!(c.next_due(), Some(t(75)));
    c.set_pause(t(50), Pause::Indefinite);
    assert_eq!(c.next_due(), None);
    c.collect(t(1000));
    c.resume(t(1000));
    c.resume(t(1002));
    assert_eq!(c.next_due(), Some(t(1020)));
    c.collect(t(1020));
    assert_eq!(c.pop().unwrap().kind, Kind::Stretch);
}
#[test]
fn changes_while_paused_affect_only_the_edited_clock() {
    let mut c = s();
    c.set_pause(t(10), Pause::Indefinite);
    c.set_interval(t(20), Kind::Stretch, ReminderInterval::Thirty);
    c.resume(t(40));
    assert_eq!(c.next_due(), Some(t(60)));
    c.set_pause(t(45), Pause::Indefinite);
    c.set_interval(t(70), Kind::Stretch, ReminderInterval::Sixty);
    c.resume(t(100));
    assert_eq!(c.next_due(), Some(t(145)));
    c.collect(t(145));
    assert_eq!(c.pop().unwrap().kind, Kind::Water);
    assert_eq!(c.next_due(), Some(t(160)));
    c.set_pause(t(150), Pause::Indefinite);
    c.set_enabled(t(150), Kind::Stretch, false);
    c.set_enabled(t(150), Kind::Water, false);
    c.manual(t(200), Kind::Stretch);
    assert_eq!(c.pop().unwrap().origin, Origin::Manual);
    assert_eq!(c.next_due(), None);
    c.set_enabled(t(210), Kind::Water, true);
    c.resume(t(300));
    assert_eq!(c.next_due(), Some(t(360)));
}
#[test]
fn exact_tie_is_held_manual_bypasses_it_without_reordering_retained_requests() {
    let mut c = s();
    c.collect(t(30));
    c.pop();
    c.set_pause(t(60), Pause::Indefinite);
    c.manual(t(61), Kind::Water);
    let r = c.pop().unwrap();
    assert_eq!(r.origin, Origin::Manual);
    assert!(c.pop().is_none());
    c.resume(t(200));
    for k in [Kind::Stretch, Kind::Water] {
        let r = c.pop().unwrap();
        assert_eq!((r.kind, r.origin, r.at), (k, Origin::Automatic, t(60)));
    }
    assert_eq!(c.next_due(), Some(t(230)));
}
#[test]
fn paused_preparation_is_retried_once_with_new_attempt_and_old_ready_is_harmless() {
    let mut e = new();
    let old = prepared(e.step(t(30), Input::Wake), Kind::Stretch);
    assert_eq!(
        e.step(t(31), Input::Pause(Pause::Indefinite)),
        [Effect::Hide(old)]
    );
    assert!(e
        .step(t(32), Input::Prepared { id: old, ok: true })
        .is_empty());
    e.step(t(33), Input::Hidden { id: old, ok: true });
    let m = prepared(e.step(t(34), Input::Manual(Kind::Water)), Kind::Water);
    show(&mut e, 34, m);
    e.step(t(46), Input::Wake);
    e.step(t(46), Input::Hidden { id: m, ok: true });
    assert!(e.step(t(50), Input::Resume).is_empty());
    let new = prepared(e.step(t(51), Input::Wake), Kind::Stretch);
    assert_ne!(new, old);
    assert!(e
        .step(t(52), Input::Hidden { id: old, ok: true })
        .is_empty());
}
#[test]
fn pause_racing_native_show_finishes_visible_or_retries_only_unseen() {
    for was_visible in [true, false] {
        let mut e = new();
        let id = prepared(e.step(t(30), Input::Wake), Kind::Stretch);
        e.step(t(30), Input::Prepared { id, ok: true });
        assert_eq!(
            e.step(t(31), Input::Pause(Pause::Indefinite)),
            [Effect::CancelShow(id)]
        );
        let result = e.step(
            t(31),
            Input::Shown {
                id,
                ok: was_visible,
            },
        );
        let end = if was_visible {
            assert_eq!(result, [Effect::Begin(id)]);
            assert_eq!(e.step(t(43), Input::Wake), [Effect::Hide(id)]);
            43
        } else {
            assert_eq!(result, [Effect::Hide(id)]);
            31
        };
        e.step(t(end), Input::Hidden { id, ok: true });
        let after = e.step(t(50), Input::Resume);
        if was_visible {
            assert!(after.is_empty());
        } else {
            assert_ne!(prepared(after, Kind::Stretch), id);
        }
    }
}
#[test]
fn disabling_during_cancelled_preparation_removes_it_even_if_reenabled() {
    let mut e = new();
    let id = prepared(e.step(t(30), Input::Wake), Kind::Stretch);
    e.step(t(30), Input::Pause(Pause::Indefinite));
    e.step(t(31), Input::ToggleAutomatic(Kind::Stretch));
    e.step(t(32), Input::ToggleAutomatic(Kind::Stretch));
    e.step(t(33), Input::Hidden { id, ok: true });
    assert!(e.step(t(40), Input::Resume).is_empty());
    assert_eq!(e.next_wake(), Some(t(70)));
}
#[test]
fn wake_barrier_must_confirm_hide_before_gap_then_new_or_retained_request() {
    for visible in [true, false] {
        let mut e = new();
        let old = prepared(e.step(t(1), Input::Manual(Kind::Water)), Kind::Water);
        if visible {
            show(&mut e, 1, old);
        }
        assert_eq!(e.step(t(3), Input::Sleep), [Effect::Hide(old)]);
        assert_eq!(e.next_wake(), None);
        e.step(t(3), Input::Manual(Kind::Stretch));
        e.step(t(3), Input::Hidden { id: old, ok: true });
        let barrier = match e.step(t(3), Input::Awaken)[..] {
            [Effect::Hide(id)] => id,
            _ => panic!(),
        };
        assert!(e.step(t(3), Input::Awaken).is_empty());
        e.step(
            t(4),
            Input::Hidden {
                id: barrier,
                ok: false,
            },
        );
        assert!(e.step(t(9), Input::Wake).is_empty());
        assert_eq!(
            e.step(t(10), Input::Manual(Kind::Water)),
            [Effect::Hide(barrier)]
        );
        e.step(
            t(11),
            Input::Hidden {
                id: barrier,
                ok: true,
            },
        );
        assert!(e.step(t(15), Input::Wake).is_empty());
        let next = prepared(
            e.step(t(16), Input::Wake),
            if visible { Kind::Stretch } else { Kind::Water },
        );
        assert_ne!(next, old);
    }
}
#[test]
fn sleeping_during_timed_pause_does_not_consume_pause_or_generate_automatic_backlog() {
    let mut e = new();
    let mut clock = Clock::new(0);
    let ns = |v| t(v).as_nanos() as u64;
    e.step(t(10), Input::Pause(Pause::Thirty));
    assert!(clock.transition(true, ns(28)));
    e.step(clock.now(ns(28)), Input::Sleep);
    assert!(clock.transition(false, ns(2000)));
    let wake = clock.now(ns(2000));
    assert_eq!(wake, t(28));
    let fx = e.step(wake, Input::Awaken);
    let [Effect::Hide(id)] = fx[..] else { panic!() };
    e.step(wake, Input::Hidden { id, ok: true });
    assert!(e.step(clock.now(ns(2011)), Input::Wake).is_empty());
    assert_eq!(e.pause(), Some(Pause::Thirty));
    e.step(clock.now(ns(2012)), Input::Wake);
    assert_eq!(e.pause(), None);
    assert_eq!(e.next_wake(), Some(t(60)));
}
#[test]
fn restore_never_replays_old_deadlines_and_keeps_only_indefinite_pause() {
    let prefs = Settings {
        stretch_interval: ReminderInterval::Sixty,
        water_interval: ReminderInterval::Sixty,
        stretch_enabled: true,
        water_enabled: false,
    };
    for indefinite in [true, false] {
        let mut e = Engine::restore(periods(), prefs, indefinite);
        assert_eq!(e.settings(), prefs);
        if indefinite {
            assert_eq!(e.next_wake(), None);
            e.step(t(900), Input::Resume);
            assert_eq!(e.next_wake(), Some(t(960)));
        } else {
            assert_eq!(e.next_wake(), Some(t(60)));
        }
    }
}
