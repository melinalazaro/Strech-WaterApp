//! Serialized scheduler/presenter. All times supplied here are awake-session times.
use crate::scheduler::{
    Kind, Origin, Pause, Periods, ReminderInterval, Request, Scheduler, Settings,
};
use std::time::Duration;
const PREPARE_LIMIT: Duration = Duration::from_secs(15);
pub(crate) const DISPLAY_TIME: Duration = Duration::from_secs(12);
const BETWEEN_REMINDERS: Duration = Duration::from_secs(5);
#[derive(Debug)]
pub enum Input {
    Wake,
    Manual(Kind),
    Interval(Kind, ReminderInterval),
    ToggleAutomatic(Kind),
    Pause(Pause),
    Resume,
    Sleep,
    Awaken,
    Prepared { id: u64, ok: bool },
    Shown { id: u64, ok: bool },
    Hidden { id: u64, ok: bool },
    Stop,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Prepare(Request),
    Show(u64),
    Begin(u64),
    CancelShow(u64),
    Hide(u64),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Preparing(Duration),
    Starting(Duration),
    Showing(Duration),
    Hiding,
    Blocked,
}
struct Active {
    request: Request,
    attempt: u64,
    phase: Phase,
    was_shown: bool,
    retain_unseen: bool,
}
pub struct Engine {
    scheduler: Scheduler,
    active: Option<Active>,
    not_before: Option<Duration>,
    stopped: bool,
    sleeping: bool,
    next_attempt: u64,
    wake_barrier: Option<(u64, bool)>,
}
impl Engine {
    pub fn new(periods: Periods) -> Self {
        Self {
            scheduler: Scheduler::new(periods),
            active: None,
            not_before: None,
            stopped: false,
            sleeping: false,
            next_attempt: 1,
            wake_barrier: None,
        }
    }
    pub fn restore(periods: Periods, settings: Settings, indefinite: bool) -> Self {
        let mut e = Self::new(periods);
        let t = Duration::ZERO;
        e.scheduler
            .set_interval(t, Kind::Stretch, settings.stretch_interval);
        e.scheduler
            .set_interval(t, Kind::Water, settings.water_interval);
        e.scheduler
            .set_enabled(t, Kind::Stretch, settings.stretch_enabled);
        e.scheduler
            .set_enabled(t, Kind::Water, settings.water_enabled);
        if indefinite {
            e.scheduler.set_pause(t, Pause::Indefinite);
        }
        e
    }
    pub fn settings(&self) -> Settings {
        self.scheduler.settings()
    }
    pub fn pause(&self) -> Option<Pause> {
        self.scheduler.pause
    }
    fn attempt(&mut self) -> u64 {
        let id = self.next_attempt;
        self.next_attempt += 1;
        id
    }
    pub fn next_wake(&self) -> Option<Duration> {
        if self.stopped || self.sleeping {
            return None;
        }
        let expiry = self.active.as_ref().and_then(|a| match a.phase {
            Phase::Preparing(t) | Phase::Starting(t) | Phase::Showing(t) => Some(t),
            _ => None,
        });
        [self.scheduler.next_due(), expiry, self.not_before]
            .into_iter()
            .flatten()
            .min()
    }
    fn finish_hidden(&mut self, now: Duration) {
        if let Some(a) = self.active.take() {
            if a.was_shown {
                self.not_before = Some(now + BETWEEN_REMINDERS);
            } else if a.retain_unseen {
                self.scheduler.restore_interrupted(a.request);
            }
        }
    }
    pub fn step(&mut self, now: Duration, input: Input) -> Vec<Effect> {
        let mut fx = vec![];
        if self.stopped {
            return fx;
        }
        if matches!(input, Input::Stop) {
            self.stopped = true;
            self.not_before = None;
            self.scheduler.clear();
            if let Some(a) = self.active.take() {
                fx.push(Effect::Hide(a.attempt));
            }
            return fx;
        }
        if self.not_before.is_some_and(|t| now >= t) {
            self.not_before = None;
        }
        if !self.sleeping {
            self.scheduler.collect(now);
        }
        match input {
            Input::Sleep if !self.sleeping => {
                self.sleeping = true;
                self.wake_barrier = None;
                if let Some(a) = &mut self.active {
                    a.retain_unseen = matches!(a.phase, Phase::Preparing(_) | Phase::Starting(_))
                        || a.retain_unseen;
                    a.phase = Phase::Hiding;
                    fx.push(Effect::Hide(a.attempt));
                }
            }
            Input::Awaken if self.sleeping => {
                self.sleeping = false;
                let id = self.attempt();
                self.wake_barrier = Some((id, false));
                fx.push(Effect::Hide(id));
            }
            Input::Manual(k) => {
                self.scheduler.manual(now, k);
                if let Some((id, true)) = self.wake_barrier {
                    self.wake_barrier = Some((id, false));
                    fx.push(Effect::Hide(id));
                } else if let Some(a) = &mut self.active {
                    if a.phase == Phase::Blocked {
                        a.phase = Phase::Hiding;
                        fx.push(Effect::Hide(a.attempt));
                    }
                }
            }
            Input::Interval(kind, v) => self.scheduler.set_interval(now, kind, v),
            Input::ToggleAutomatic(k) => {
                let on = !self.scheduler.enabled(k);
                self.scheduler.set_enabled(now, k, on);
                if !on {
                    if let Some(a) = &mut self.active {
                        if a.request.kind == k && a.request.origin == Origin::Automatic {
                            a.retain_unseen = false;
                        }
                    }
                }
            }
            Input::Pause(kind) => {
                self.scheduler.set_pause(now, kind);
                if let Some(a) = &mut self.active {
                    if a.request.origin == Origin::Automatic {
                        match a.phase {
                            Phase::Preparing(_) => {
                                a.retain_unseen = true;
                                a.phase = Phase::Hiding;
                                fx.push(Effect::Hide(a.attempt));
                            }
                            Phase::Starting(_) => {
                                a.retain_unseen = true;
                                fx.push(Effect::CancelShow(a.attempt));
                            }
                            _ => {}
                        }
                    }
                }
            }
            Input::Resume => self.scheduler.resume(now),
            Input::Prepared { id, ok } => {
                if let Some(a) = &mut self.active {
                    if a.attempt == id && matches!(a.phase, Phase::Preparing(_)) {
                        if ok {
                            a.phase = Phase::Starting(now + PREPARE_LIMIT);
                            fx.push(Effect::Show(id));
                        } else {
                            a.phase = Phase::Hiding;
                            fx.push(Effect::Hide(id));
                        }
                    }
                }
            }
            Input::Shown { id, ok } => {
                if let Some(a) = &mut self.active {
                    if a.attempt == id {
                        a.was_shown |= ok;
                        if let Phase::Starting(deadline) = a.phase {
                            if ok && now < deadline {
                                a.phase = Phase::Showing(now + DISPLAY_TIME);
                                fx.push(Effect::Begin(id));
                            } else {
                                a.phase = Phase::Hiding;
                                fx.push(Effect::Hide(id));
                            }
                        }
                    }
                }
            }
            Input::Hidden { id, ok } if self.wake_barrier.is_some_and(|(token, _)| id == token) => {
                if ok {
                    self.finish_hidden(now);
                    self.wake_barrier = None;
                    self.not_before = Some(now + BETWEEN_REMINDERS);
                } else {
                    self.wake_barrier = Some((id, true));
                }
            }
            Input::Hidden { id, ok }
                if self.active.as_ref().is_some_and(|a| {
                    a.attempt == id && matches!(a.phase, Phase::Hiding | Phase::Blocked)
                }) =>
            {
                if ok {
                    self.finish_hidden(now);
                } else if let Some(a) = &mut self.active {
                    a.phase = Phase::Blocked;
                }
            }
            _ => {}
        }
        if self.sleeping {
            return fx;
        }
        if let Some(a) = &mut self.active {
            if matches!(a.phase,Phase::Preparing(t)|Phase::Starting(t)|Phase::Showing(t) if now>=t)
            {
                a.phase = Phase::Hiding;
                fx.push(Effect::Hide(a.attempt));
            }
        }
        if self.active.is_none() && self.not_before.is_none() && self.wake_barrier.is_none() {
            if let Some(request) = self.scheduler.pop() {
                let id = self.attempt();
                let mut payload = request.clone();
                payload.id = id;
                fx.push(Effect::Prepare(payload));
                self.active = Some(Active {
                    attempt: id,
                    request,
                    phase: Phase::Preparing(now + PREPARE_LIMIT),
                    was_shown: false,
                    retain_unseen: false,
                });
            }
        }
        fx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }
    fn engine() -> Engine {
        Engine::new(Periods::normal())
    }
    #[test]
    fn two_hour_reminders_keep_display_duration_and_gap() {
        let mut e = engine();
        for kind in [Kind::Stretch, Kind::Water] {
            e.step(s(0), Input::Interval(kind, ReminderInterval::TwoHours));
        }
        assert_eq!(e.next_wake(), Some(s(7200)));
        let first = e.step(s(7200), Input::Wake);
        assert!(matches!(&first[..], [Effect::Prepare(r)] if r.kind == Kind::Stretch));
        let first_id = id(&first[0]);
        shown(&mut e, 7200, first_id);
        e.step(
            s(7201),
            Input::Interval(Kind::Water, ReminderInterval::Thirty),
        );
        assert!(e.step(s(7211), Input::Wake).is_empty());
        assert_eq!(e.step(s(7212), Input::Wake), [Effect::Hide(first_id)]);
        e.step(
            s(7212),
            Input::Hidden {
                id: first_id,
                ok: true,
            },
        );
        assert!(e.step(s(7216), Input::Wake).is_empty());
        assert!(matches!(&e.step(s(7217), Input::Wake)[..],
            [Effect::Prepare(r)] if r.kind == Kind::Water && r.at == s(7200)));
    }
    fn id(e: &Effect) -> u64 {
        match e {
            Effect::Prepare(r) => r.id,
            _ => panic!("Expected prepare"),
        }
    }
    fn shown(e: &mut Engine, at: u64, id: u64) {
        assert_eq!(
            e.step(s(at), Input::Prepared { id, ok: true }),
            vec![Effect::Show(id)]
        );
        assert_eq!(
            e.step(s(at), Input::Shown { id, ok: true }),
            [Effect::Begin(id)]
        );
    }
    #[test]
    fn media_starts_only_once_per_accepted_show() {
        let mut e = engine();
        let first = id(&e.step(s(0), Input::Manual(Kind::Stretch))[0]);
        e.step(
            s(0),
            Input::Prepared {
                id: first,
                ok: true,
            },
        );
        let shown = || Input::Shown {
            id: first,
            ok: true,
        };
        assert_eq!(e.step(s(1), shown()), [Effect::Begin(first)]);
        assert!(e.step(s(2), shown()).is_empty());
        assert_eq!(e.step(s(3), Input::Sleep), [Effect::Hide(first)]);
        assert!(e.step(s(4), shown()).is_empty());
        e.step(s(5), Input::Stop);
        assert!(e.step(s(6), shown()).is_empty());
    }
    #[test]
    fn confirmation_after_start_timeout_cannot_play_audio() {
        let mut e = engine();
        let first = id(&e.step(s(0), Input::Manual(Kind::Water))[0]);
        e.step(
            s(0),
            Input::Prepared {
                id: first,
                ok: true,
            },
        );
        assert_eq!(
            e.step(
                s(16),
                Input::Shown {
                    id: first,
                    ok: true
                }
            ),
            [Effect::Hide(first)]
        );
    }
    #[test]
    fn loading_is_busy_and_next_waits_five_seconds_after_confirmed_hide() {
        let mut e = engine();
        let first = id(&e.step(s(0), Input::Manual(Kind::Water))[0]);
        assert!(e.step(s(1), Input::Manual(Kind::Stretch)).is_empty());
        shown(&mut e, 2, first);
        assert!(e.step(s(13), Input::Wake).is_empty());
        assert_eq!(e.step(s(14), Input::Wake), vec![Effect::Hide(first)]);
        assert!(e
            .step(
                s(15),
                Input::Hidden {
                    id: first,
                    ok: true
                }
            )
            .is_empty());
        assert_eq!(e.next_wake(), Some(s(20)));
        assert!(e.step(s(19), Input::Wake).is_empty());
        assert!(
            matches!(&e.step(s(20),Input::Wake)[..],[Effect::Prepare(r)] if r.kind==Kind::Stretch)
        );
    }
    #[test]
    fn repeated_requests_preserve_fifo_and_stale_results_do_not_extend_gap() {
        let mut e = engine();
        e.step(s(0), Input::Manual(Kind::Stretch));
        e.step(s(1), Input::Manual(Kind::Water));
        e.step(s(2), Input::Manual(Kind::Stretch));
        shown(&mut e, 3, 1);
        assert!(e.step(s(4), Input::Hidden { id: 1, ok: true }).is_empty());
        e.step(s(15), Input::Wake);
        assert!(e.step(s(15), Input::Hidden { id: 1, ok: true }).is_empty());
        assert!(e.step(s(16), Input::Hidden { id: 1, ok: true }).is_empty());
        assert!(e
            .step(s(16), Input::Prepared { id: 1, ok: true })
            .is_empty());
        assert_eq!(e.next_wake(), Some(s(20)));
        let next = e.step(s(20), Input::Wake);
        let second = id(&next[0]);
        shown(&mut e, 20, second);
        e.step(s(32), Input::Wake);
        e.step(
            s(32),
            Input::Hidden {
                id: second,
                ok: true,
            },
        );
        assert!(
            matches!(&e.step(s(37),Input::Wake)[..],[Effect::Prepare(r)] if r.kind==Kind::Stretch)
        );
    }
    #[test]
    fn hide_failure_blocks_until_retry_and_then_starts_gap() {
        let mut e = engine();
        e.step(s(0), Input::Manual(Kind::Water));
        shown(&mut e, 1, 1);
        e.step(s(2), Input::Manual(Kind::Stretch));
        e.step(s(13), Input::Wake);
        assert!(e.step(s(13), Input::Hidden { id: 1, ok: false }).is_empty());
        assert!(e.step(s(14), Input::Wake).is_empty());
        assert_eq!(
            e.step(s(14), Input::Manual(Kind::Water)),
            vec![Effect::Hide(1)]
        );
        assert!(e.step(s(14), Input::Hidden { id: 1, ok: true }).is_empty());
        assert!(
            matches!(&e.step(s(19),Input::Wake)[..],[Effect::Prepare(r)] if r.kind==Kind::Stretch)
        );
    }
    #[test]
    fn failed_preparation_without_visible_window_does_not_add_gap() {
        for failure in [Input::Prepared { id: 1, ok: false }, Input::Wake] {
            let mut e = engine();
            e.step(s(0), Input::Manual(Kind::Stretch));
            e.step(s(1), Input::Manual(Kind::Water));
            assert_eq!(e.step(s(15), failure), vec![Effect::Hide(1)]);
            assert!(
                matches!(&e.step(s(15),Input::Hidden{id:1,ok:true})[..],[Effect::Prepare(r)] if r.kind==Kind::Water)
            );
        }
    }
    #[test]
    fn show_failure_and_late_success_wait_for_hide_and_gap() {
        let mut e = engine();
        e.step(s(0), Input::Manual(Kind::Water));
        e.step(s(1), Input::Manual(Kind::Stretch));
        e.step(s(1), Input::Prepared { id: 1, ok: true });
        assert_eq!(
            e.step(s(2), Input::Shown { id: 1, ok: false }),
            vec![Effect::Hide(1)]
        );
        assert!(e.step(s(3), Input::Shown { id: 1, ok: true }).is_empty());
        assert!(e.step(s(4), Input::Hidden { id: 1, ok: true }).is_empty());
        assert_eq!(e.next_wake(), Some(s(9)));
    }
    #[test]
    fn automatic_due_during_showing_waits_without_calendar_drift() {
        let mut e = engine();
        e.step(s(1799), Input::Manual(Kind::Water));
        shown(&mut e, 1799, 1);
        assert!(e.step(s(1800), Input::Wake).is_empty());
        e.step(s(1811), Input::Wake);
        assert!(e
            .step(s(1811), Input::Hidden { id: 1, ok: true })
            .is_empty());
        assert!(
            matches!(&e.step(s(1816),Input::Wake)[..],[Effect::Prepare(r)] if r.kind==Kind::Stretch && r.at==s(1800))
        );
        assert_eq!(e.scheduler.next_due(), Some(s(3600)));
    }
    #[test]
    fn changing_interval_preserves_active_and_queued_requests() {
        let mut e = engine();
        e.step(s(0), Input::Manual(Kind::Water));
        e.step(s(1), Input::Manual(Kind::Stretch));
        assert!(e
            .step(
                s(2),
                Input::Interval(Kind::Stretch, ReminderInterval::Sixty)
            )
            .is_empty());
        assert_eq!(e.scheduler.next_due(), Some(s(3600)));
        shown(&mut e, 3, 1);
        e.step(s(15), Input::Wake);
        e.step(s(15), Input::Hidden { id: 1, ok: true });
        assert!(
            matches!(&e.step(s(20),Input::Wake)[..],[Effect::Prepare(r)] if r.kind==Kind::Stretch)
        );
    }
    #[test]
    fn stop_cancels_active_or_gap_and_ignores_late_messages() {
        for stop_at in [1, 13] {
            let mut e = engine();
            e.step(s(0), Input::Manual(Kind::Stretch));
            e.step(s(1), Input::Manual(Kind::Water));
            if stop_at == 13 {
                shown(&mut e, 1, 1);
                e.step(s(13), Input::Wake);
                e.step(s(13), Input::Hidden { id: 1, ok: true });
            }
            e.step(s(stop_at), Input::Stop);
            assert_eq!(e.next_wake(), None);
            for input in [
                Input::Wake,
                Input::Prepared { id: 1, ok: true },
                Input::Manual(Kind::Water),
                Input::ToggleAutomatic(Kind::Stretch),
            ] {
                assert!(e.step(s(7200), input).is_empty());
            }
        }
    }
    #[test]
    fn manual_arriving_in_gap_waits_even_if_queue_was_empty() {
        let mut e = engine();
        e.step(s(0), Input::Manual(Kind::Water));
        shown(&mut e, 0, 1);
        e.step(s(12), Input::Wake);
        e.step(s(12), Input::Hidden { id: 1, ok: true });
        assert!(e.step(s(14), Input::Manual(Kind::Water)).is_empty());
        assert!(e.step(s(16), Input::Wake).is_empty());
        assert!(
            matches!(&e.step(s(17),Input::Wake)[..],[Effect::Prepare(r)] if r.kind==Kind::Water)
        );
    }
    #[test]
    fn expired_empty_gap_is_cleared_without_busy_wake() {
        let mut e = engine();
        e.step(s(0), Input::Manual(Kind::Water));
        shown(&mut e, 0, 1);
        e.step(s(12), Input::Wake);
        e.step(s(12), Input::Hidden { id: 1, ok: true });
        assert_eq!(e.next_wake(), Some(s(17)));
        assert!(e.step(s(17), Input::Wake).is_empty());
        assert_eq!(e.next_wake(), Some(s(1800)));
    }
    #[test]
    fn late_hide_confirmation_starts_gap_from_actual_confirmation() {
        let mut e = engine();
        e.step(s(0), Input::Manual(Kind::Water));
        shown(&mut e, 0, 1);
        e.step(s(1), Input::Manual(Kind::Stretch));
        e.step(s(12), Input::Wake);
        assert!(e.step(s(20), Input::Hidden { id: 1, ok: true }).is_empty());
        assert_eq!(e.next_wake(), Some(s(25)));
    }
    #[test]
    fn both_disabled_still_allow_manual_gap_and_reactivation() {
        let mut e = engine();
        e.step(s(0), Input::ToggleAutomatic(Kind::Stretch));
        e.step(s(0), Input::ToggleAutomatic(Kind::Water));
        assert_eq!(e.next_wake(), None);
        assert!(!e.settings().water_enabled);
        e.step(s(0), Input::Manual(Kind::Water));
        shown(&mut e, 0, 1);
        e.step(s(12), Input::Wake);
        e.step(s(12), Input::Hidden { id: 1, ok: true });
        e.step(s(17), Input::Wake);
        assert_eq!(e.next_wake(), None);
        e.step(s(20), Input::ToggleAutomatic(Kind::Water));
        assert_eq!(e.next_wake(), Some(s(3620)));
    }
    #[test]
    fn disabling_keeps_active_request_and_cancels_queued_automatic_only() {
        let mut e = engine();
        e.step(s(3600), Input::Wake);
        shown(&mut e, 3600, 1);
        e.step(s(3601), Input::Manual(Kind::Stretch));
        e.step(s(3602), Input::ToggleAutomatic(Kind::Stretch));
        assert!(!e.settings().stretch_enabled);
        assert!(matches!(
            e.active.as_ref().unwrap().phase,
            Phase::Showing(_)
        ));
        e.step(s(3612), Input::Wake);
        e.step(s(3612), Input::Hidden { id: 1, ok: true });
        assert!(
            matches!(&e.step(s(3617),Input::Wake)[..],[Effect::Prepare(r)] if r.kind==Kind::Water)
        );
    }

    #[test]
    fn automatic_due_inside_gap_is_queued_without_busy_wake_or_calendar_drift() {
        let mut e = engine();
        e.step(s(1786), Input::Manual(Kind::Water));
        shown(&mut e, 1786, 1);
        e.step(s(1798), Input::Wake);
        e.step(s(1798), Input::Hidden { id: 1, ok: true });
        assert_eq!(e.next_wake(), Some(s(1800)));
        assert!(e.step(s(1800), Input::Wake).is_empty());
        assert_eq!(e.next_wake(), Some(s(1803)));
        assert!(
            matches!(&e.step(s(1803),Input::Wake)[..],[Effect::Prepare(r)] if r.kind==Kind::Stretch && r.at==s(1800))
        );
        assert_eq!(e.scheduler.next_due(), Some(s(3600)));
    }
}
