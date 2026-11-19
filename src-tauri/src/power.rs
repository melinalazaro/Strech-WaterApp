//! Clock in awake nanoseconds. The explicit sleep interval also excludes dispatch
//! overhead between will-sleep and did-wake; wall time never enters the engine.
use crate::runtime::Channel;
use std::{sync::OnceLock, time::Duration};
extern "C" {
    fn sw_awake_ns() -> u64;
    fn sw_start_error(detail: *const std::ffi::c_char);
    fn sw_power_start(cb: extern "C" fn(i32, u64));
}
pub fn startup_error(detail: String) {
    if let Ok(s) = std::ffi::CString::new(detail) {
        unsafe { sw_start_error(s.as_ptr()) }
    }
}
pub fn raw() -> u64 {
    unsafe { sw_awake_ns() }
}
static CHANNEL: OnceLock<Channel> = OnceLock::new();
extern "C" fn event(asleep: i32, stamp: u64) {
    if let Some(ch) = CHANNEL.get() {
        if asleep == 2 {
            ch.stop();
        } else {
            ch.power(asleep != 0, stamp);
        }
    }
}
pub fn install(ch: Channel) {
    if CHANNEL.set(ch).is_ok() {
        unsafe { sw_power_start(event) }
    }
}
pub struct Clock {
    origin: u64,
    excluded: u64,
    since: Option<u64>,
    last_event: u64,
}
impl Clock {
    pub fn new(origin: u64) -> Self {
        Self {
            origin,
            excluded: 0,
            since: None,
            last_event: origin,
        }
    }
    pub fn now(&self, t: u64) -> Duration {
        Duration::from_nanos(
            self.since
                .unwrap_or(t)
                .saturating_sub(self.origin)
                .saturating_sub(self.excluded),
        )
    }
    pub fn transition(&mut self, asleep: bool, stamp: u64) -> bool {
        if stamp < self.last_event || self.since.is_some() == asleep {
            return false;
        }
        self.last_event = stamp;
        if asleep {
            self.since = Some(stamp);
        } else if let Some(start) = self.since.take() {
            self.excluded += stamp.saturating_sub(start);
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freeze_preserves_time_across_long_sleep_and_ignores_duplicates() {
        let mut c = Clock::new(100);
        assert!(c.transition(true, 120));
        assert!(!c.transition(true, 130));
        assert_eq!(c.now(90120).as_nanos(), 20);
        assert!(!c.transition(false, 110));
        assert!(c.transition(false, 90120));
        assert!(!c.transition(false, 90121));
        assert_eq!(c.now(90125).as_nanos(), 25);
    }
}
