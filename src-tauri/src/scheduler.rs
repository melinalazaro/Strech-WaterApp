//! Domain logic: explicit elapsed time, no OS, Tauri, or wall-clock access.
use std::{collections::VecDeque, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Stretch,
    Water,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReminderInterval {
    Thirty,
    Sixty,
    TwoHours,
}
impl ReminderInterval {
    pub fn minutes(self) -> u32 {
        match self {
            Self::Thirty => 30,
            Self::Sixty => 60,
            Self::TwoHours => 120,
        }
    }
}
impl TryFrom<u32> for ReminderInterval {
    type Error = &'static str;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            30 => Ok(Self::Thirty),
            60 => Ok(Self::Sixty),
            120 => Ok(Self::TwoHours),
            _ => Err("Solo 30, 60 o 120 minutos"),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Automatic,
    Manual,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub id: u64,
    pub kind: Kind,
    pub origin: Origin,
    pub at: Duration,
}
#[derive(Clone, Copy)]
pub struct Periods {
    pub thirty: Duration,
    pub sixty: Duration,
}
impl Periods {
    pub fn normal() -> Self {
        Self {
            thirty: Duration::from_secs(1800),
            sixty: Duration::from_secs(3600),
        }
    }
    pub fn duration(self, interval: ReminderInterval) -> Duration {
        match interval {
            ReminderInterval::Thirty => self.thirty,
            ReminderInterval::Sixty => self.sixty,
            ReminderInterval::TwoHours => self.sixty * 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub stretch_interval: ReminderInterval,
    pub water_interval: ReminderInterval,
    pub stretch_enabled: bool,
    pub water_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pause {
    Thirty,
    Sixty,
    Indefinite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Calendar {
    Off,
    Due(Duration),
    Frozen(Duration),
}
impl Calendar {
    fn due(self) -> Option<Duration> {
        match self {
            Self::Due(t) => Some(t),
            _ => None,
        }
    }
}
pub struct Scheduler {
    periods: Periods,
    stretch_interval: ReminderInterval,
    water_interval: ReminderInterval,
    stretch: Calendar,
    water: Calendar,
    pub pause: Option<Pause>,
    resume_at: Option<Duration>,
    next_id: u64,
    queue: VecDeque<Request>,
}
impl Scheduler {
    pub fn new(periods: Periods) -> Self {
        Self {
            periods,
            stretch_interval: ReminderInterval::Thirty,
            water_interval: ReminderInterval::Sixty,
            stretch: Calendar::Due(periods.thirty),
            water: Calendar::Due(periods.sixty),
            pause: None,
            resume_at: None,
            next_id: 1,
            queue: VecDeque::new(),
        }
    }
    fn calendar(&mut self, k: Kind) -> &mut Calendar {
        match k {
            Kind::Stretch => &mut self.stretch,
            Kind::Water => &mut self.water,
        }
    }
    fn period(&self, k: Kind) -> Duration {
        self.periods.duration(match k {
            Kind::Stretch => self.stretch_interval,
            Kind::Water => self.water_interval,
        })
    }
    pub fn next_due(&self) -> Option<Duration> {
        [self.stretch.due(), self.water.due(), self.resume_at]
            .into_iter()
            .flatten()
            .min()
    }
    pub fn enabled(&self, k: Kind) -> bool {
        (match k {
            Kind::Stretch => self.stretch,
            Kind::Water => self.water,
        }) != Calendar::Off
    }
    pub fn settings(&self) -> Settings {
        Settings {
            stretch_interval: self.stretch_interval,
            water_interval: self.water_interval,
            stretch_enabled: self.enabled(Kind::Stretch),
            water_enabled: self.enabled(Kind::Water),
        }
    }
    pub fn collect(&mut self, now: Duration) {
        if let Some(at) = self.resume_at.filter(|at| *at <= now) {
            self.resume(at);
        }
        if self.pause.is_some() {
            return;
        }
        while let Some(due) = self.next_due().filter(|t| *t <= now) {
            let k = if self.stretch.due() == Some(due) {
                Kind::Stretch
            } else {
                Kind::Water
            };
            *self.calendar(k) = Calendar::Due(due + self.period(k));
            self.push(k, Origin::Automatic, due);
        }
    }
    fn push(&mut self, kind: Kind, origin: Origin, at: Duration) {
        self.queue.push_back(Request {
            id: self.next_id,
            kind,
            origin,
            at,
        });
        self.next_id += 1;
    }
    pub fn set_pause(&mut self, now: Duration, kind: Pause) {
        self.collect(now);
        for cal in [&mut self.stretch, &mut self.water] {
            if let Calendar::Due(t) = *cal {
                *cal = Calendar::Frozen(t.saturating_sub(now));
            }
        }
        self.pause = Some(kind);
        self.resume_at = match kind {
            Pause::Thirty => Some(now + self.periods.thirty),
            Pause::Sixty => Some(now + self.periods.sixty),
            Pause::Indefinite => None,
        };
    }
    pub fn resume(&mut self, now: Duration) {
        for cal in [&mut self.stretch, &mut self.water] {
            if let Calendar::Frozen(left) = *cal {
                *cal = Calendar::Due(now + left);
            }
        }
        self.pause = None;
        self.resume_at = None;
    }
    fn full(&self, now: Duration, k: Kind) -> Calendar {
        if self.pause.is_some() {
            Calendar::Frozen(self.period(k))
        } else {
            Calendar::Due(now + self.period(k))
        }
    }
    pub fn manual(&mut self, now: Duration, k: Kind) {
        self.collect(now);
        self.push(k, Origin::Manual, now);
    }
    pub fn set_interval(&mut self, now: Duration, kind: Kind, v: ReminderInterval) {
        self.collect(now);
        let interval = match kind {
            Kind::Stretch => &mut self.stretch_interval,
            Kind::Water => &mut self.water_interval,
        };
        if *interval != v {
            *interval = v;
            if self.enabled(kind) {
                *self.calendar(kind) = self.full(now, kind);
            }
        }
    }
    pub fn set_enabled(&mut self, now: Duration, k: Kind, on: bool) {
        self.collect(now);
        if self.enabled(k) == on {
            return;
        }
        *self.calendar(k) = if on { self.full(now, k) } else { Calendar::Off };
        if !on {
            self.queue
                .retain(|r| r.kind != k || r.origin == Origin::Manual);
        }
    }
    pub fn pop(&mut self) -> Option<Request> {
        if self.pause.is_some() {
            let index = self.queue.iter().position(|r| r.origin == Origin::Manual)?;
            self.queue.remove(index)
        } else {
            self.queue.pop_front()
        }
    }
    pub fn restore_interrupted(&mut self, r: Request) {
        if r.origin == Origin::Manual || self.enabled(r.kind) {
            self.queue.push_front(r);
        }
    }
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(n: u64) -> Duration {
        Duration::from_secs(n * 60)
    }
    fn scheduler() -> Scheduler {
        Scheduler::new(Periods::normal())
    }
    fn drain(s: &mut Scheduler) -> Vec<(Kind, Origin, Duration)> {
        std::iter::from_fn(|| s.pop())
            .map(|r| (r.kind, r.origin, r.at))
            .collect()
    }
    #[test]
    fn first_deadline_and_exact_boundaries() {
        let mut s = scheduler();
        s.collect(m(30) - Duration::from_nanos(1));
        assert!(s.pop().is_none());
        s.collect(m(30));
        let r = s.pop().unwrap();
        assert_eq!((r.kind, r.at), (Kind::Stretch, m(30)));
        assert_eq!(s.next_due(), Some(m(60)));
    }
    #[test]
    fn cycles_are_ordered_by_deadline_then_tie_priority() {
        let mut s = scheduler();
        s.collect(m(120));
        assert_eq!(
            drain(&mut s),
            vec![
                (Kind::Stretch, Origin::Automatic, m(30)),
                (Kind::Stretch, Origin::Automatic, m(60)),
                (Kind::Water, Origin::Automatic, m(60)),
                (Kind::Stretch, Origin::Automatic, m(90)),
                (Kind::Stretch, Origin::Automatic, m(120)),
                (Kind::Water, Origin::Automatic, m(120))
            ]
        );
    }
    #[test]
    fn manuals_preserve_both_deadlines_and_fifo() {
        let mut s = scheduler();
        s.manual(m(29), Kind::Water);
        s.manual(m(29), Kind::Stretch);
        assert_eq!(s.next_due(), Some(m(30)));
        s.collect(m(60));
        let result = drain(&mut s);
        assert_eq!(
            result.iter().map(|x| x.0).collect::<Vec<_>>(),
            vec![
                Kind::Water,
                Kind::Stretch,
                Kind::Stretch,
                Kind::Stretch,
                Kind::Water
            ]
        );
        assert_eq!(result[2].2, m(30));
        assert_eq!(result[4].2, m(60));
    }
    #[test]
    fn automatic_due_is_collected_before_manual_at_same_time() {
        let mut s = scheduler();
        s.collect(m(30));
        s.pop();
        s.manual(m(60), Kind::Water);
        assert_eq!(
            drain(&mut s),
            vec![
                (Kind::Stretch, Origin::Automatic, m(60)),
                (Kind::Water, Origin::Automatic, m(60)),
                (Kind::Water, Origin::Manual, m(60))
            ]
        );
    }
    #[test]
    fn interval_change_preserves_water_and_existing_queue() {
        let mut s = scheduler();
        s.manual(m(10), Kind::Water);
        s.set_interval(m(20), Kind::Stretch, ReminderInterval::Sixty);
        assert_eq!(s.next_due(), Some(m(60)));
        s.collect(m(80));
        assert_eq!(
            drain(&mut s),
            vec![
                (Kind::Water, Origin::Manual, m(10)),
                (Kind::Water, Origin::Automatic, m(60)),
                (Kind::Stretch, Origin::Automatic, m(80))
            ]
        );
    }
    #[test]
    fn same_selection_does_not_restart_and_sixty_ties_work() {
        let mut s = scheduler();
        s.set_interval(m(20), Kind::Stretch, ReminderInterval::Thirty);
        assert_eq!(s.next_due(), Some(m(30)));
        let mut s = scheduler();
        s.set_interval(Duration::ZERO, Kind::Stretch, ReminderInterval::Sixty);
        s.collect(m(60));
        assert_eq!(
            drain(&mut s).iter().map(|x| x.0).collect::<Vec<_>>(),
            vec![Kind::Stretch, Kind::Water]
        );
    }
    #[test]
    fn invalid_intervals_rejected() {
        for n in [0, 1, 29, 31, 59, 61, 90] {
            assert!(ReminderInterval::try_from(n).is_err());
        }
        assert_eq!(ReminderInterval::try_from(60), Ok(ReminderInterval::Sixty));
        assert_eq!(
            ReminderInterval::try_from(120),
            Ok(ReminderInterval::TwoHours)
        );
    }

    #[test]
    fn two_hour_intervals_have_exact_boundaries_in_both_time_scales() {
        for (periods, deadline) in [
            (Periods::normal(), m(120)),
            (
                Periods {
                    thirty: Duration::from_secs(30),
                    sixty: Duration::from_secs(60),
                },
                Duration::from_secs(120),
            ),
        ] {
            let mut s = Scheduler::new(periods);
            for kind in [Kind::Stretch, Kind::Water] {
                s.set_interval(Duration::ZERO, kind, ReminderInterval::TwoHours);
            }
            for kind in [Kind::Stretch, Kind::Water] {
                s.set_interval(deadline / 2, kind, ReminderInterval::TwoHours);
            }
            s.collect(deadline - Duration::from_nanos(1));
            assert!(s.pop().is_none());
            s.collect(deadline);
            assert_eq!(
                drain(&mut s),
                [
                    (Kind::Stretch, Origin::Automatic, deadline),
                    (Kind::Water, Origin::Automatic, deadline),
                ]
            );
            assert_eq!(s.next_due(), Some(deadline * 2));
        }
    }

    #[test]
    fn water_interval_changes_preserve_stretch_and_queued_requests() {
        let mut s = scheduler();
        s.manual(m(10), Kind::Water);
        s.set_interval(m(20), Kind::Water, ReminderInterval::Thirty);
        s.collect(m(60));
        s.set_interval(m(60), Kind::Water, ReminderInterval::TwoHours);
        assert_eq!(
            drain(&mut s),
            [
                (Kind::Water, Origin::Manual, m(10)),
                (Kind::Stretch, Origin::Automatic, m(30)),
                (Kind::Water, Origin::Automatic, m(50)),
                (Kind::Stretch, Origin::Automatic, m(60)),
            ]
        );
        assert_eq!(s.next_due(), Some(m(90)));
        s.set_enabled(m(60), Kind::Stretch, false);
        assert_eq!(s.next_due(), Some(m(180)));
    }

    #[test]
    fn water_selection_during_pause_and_while_disabled_keeps_full_interval() {
        let mut s = scheduler();
        for kind in [Kind::Stretch, Kind::Water] {
            s.set_interval(m(0), kind, ReminderInterval::TwoHours);
        }
        s.set_pause(m(30), Pause::Indefinite);
        s.set_interval(m(40), Kind::Water, ReminderInterval::Thirty);
        s.set_interval(m(50), Kind::Water, ReminderInterval::Thirty);
        assert_eq!(s.next_due(), None);
        s.resume(m(100));
        s.collect(m(130));
        assert_eq!(drain(&mut s), [(Kind::Water, Origin::Automatic, m(130))]);
        s.set_enabled(m(130), Kind::Water, false);
        s.set_interval(m(140), Kind::Water, ReminderInterval::TwoHours);
        assert!(!s.enabled(Kind::Water));
        assert_eq!(s.next_due(), Some(m(190)));
        s.set_enabled(m(150), Kind::Water, true);
        s.collect(m(270));
        assert_eq!(
            drain(&mut s),
            [
                (Kind::Stretch, Origin::Automatic, m(190)),
                (Kind::Water, Origin::Automatic, m(270)),
            ]
        );
    }

    #[test]
    fn disabling_purges_only_automatic_pending_requests_of_that_kind() {
        let mut s = scheduler();
        s.manual(m(29), Kind::Stretch);
        s.manual(m(29), Kind::Water);
        s.collect(m(60));
        s.set_enabled(m(60), Kind::Stretch, false);
        assert_eq!(
            drain(&mut s),
            vec![
                (Kind::Stretch, Origin::Manual, m(29)),
                (Kind::Water, Origin::Manual, m(29)),
                (Kind::Water, Origin::Automatic, m(60))
            ]
        );
        assert_eq!(s.next_due(), Some(m(120)));
    }
    #[test]
    fn reenable_starts_full_interval_without_catch_up_or_changing_other_type() {
        let mut s = scheduler();
        s.set_enabled(m(10), Kind::Stretch, false);
        s.collect(m(120));
        drain(&mut s);
        s.set_enabled(m(125), Kind::Stretch, true);
        assert_eq!(s.next_due(), Some(m(155)));
        s.collect(m(180));
        assert_eq!(
            drain(&mut s),
            vec![
                (Kind::Stretch, Origin::Automatic, m(155)),
                (Kind::Water, Origin::Automatic, m(180))
            ]
        );
    }
    #[test]
    fn changing_interval_while_disabled_does_not_enable_or_schedule_it() {
        let mut s = scheduler();
        s.set_enabled(m(10), Kind::Stretch, false);
        s.set_interval(m(20), Kind::Stretch, ReminderInterval::Sixty);
        assert!(!s.settings().stretch_enabled);
        assert_eq!(s.next_due(), Some(m(60)));
        s.set_enabled(m(25), Kind::Stretch, true);
        s.collect(m(85));
        assert_eq!(
            drain(&mut s),
            vec![
                (Kind::Water, Origin::Automatic, m(60)),
                (Kind::Stretch, Origin::Automatic, m(85))
            ]
        );
    }
    #[test]
    fn both_disabled_keep_manual_actions_available_and_no_deadline() {
        let mut s = scheduler();
        s.set_enabled(m(0), Kind::Stretch, false);
        s.set_enabled(m(0), Kind::Water, false);
        s.collect(m(240));
        assert_eq!(s.next_due(), None);
        assert!(s.pop().is_none());
        s.manual(m(240), Kind::Water);
        assert_eq!(drain(&mut s), vec![(Kind::Water, Origin::Manual, m(240))]);
    }
    #[test]
    fn enabling_same_state_is_idempotent_and_water_stays_at_sixty() {
        let mut s = scheduler();
        s.set_enabled(m(10), Kind::Stretch, true);
        assert_eq!(s.next_due(), Some(m(30)));
        s.set_enabled(m(10), Kind::Water, false);
        s.set_enabled(m(15), Kind::Water, true);
        s.collect(m(75));
        assert_eq!(
            drain(&mut s).last(),
            Some(&(Kind::Water, Origin::Automatic, m(75)))
        );
    }
}
