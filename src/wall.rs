//! Wall-clock (calendar) time — the standard counterpart to this crate's
//! tick-based time. In terms of a broader "levels of time" sketch, this
//! is "duration in absolute units" (seconds), as opposed to tick-counting
//! ("before/after").
//!
//! This does not replace tick-based time — many systems genuinely have
//! no clock of their own, and ticks remain the primary notion of time for
//! them. This module is a separate, optional metric for the cases where
//! real time is still useful (logs, timeouts, correlating with events
//! outside the system). Deliberately not implemented: relativistic,
//! branching, looping, or metaphysical notions of time — speculative
//! territory for future architectures in general, not something real
//! code expresses today.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant, SystemTime};

/// The current moment as calendar time — for logs and correlating with
/// the outside world. Do not use this to measure durations (the system
/// clock can jump backwards, e.g. during NTP sync) — use `RealTouchLog`
/// for that, which is built on `Instant` (a monotonic clock).
pub fn now_wall_clock() -> SystemTime {
    SystemTime::now()
}

/// A real-time log of last touches — the same contract as the tick-based
/// `TouchLog`, but in `Instant`/`Duration` rather than ticks. Built on
/// `Instant` (monotonic), not `SystemTime` — resilient to system clock
/// jumps, suitable for measuring elapsed duration.
#[derive(Default)]
pub struct RealTouchLog<Id> {
    last_touched: HashMap<Id, Instant>,
}

impl<Id: Eq + Hash + Copy> RealTouchLog<Id> {
    pub fn new() -> Self {
        Self { last_touched: HashMap::new() }
    }

    /// Record a touch right now (real clock).
    pub fn touch(&mut self, id: Id) {
        self.last_touched.insert(id, Instant::now());
    }

    /// Record a touch at an explicitly given moment — for reproducible
    /// tests, and for cases where the touch moment is known but isn't
    /// "right now" (e.g. restored from a log).
    pub fn touch_at(&mut self, id: Id, at: Instant) {
        self.last_touched.insert(id, at);
    }

    pub fn last_touched(&self, id: Id) -> Option<Instant> {
        self.last_touched.get(&id).copied()
    }

    /// Real elapsed duration since the last touch, up to now. `None` if
    /// the id was never touched.
    pub fn age(&self, id: Id) -> Option<Duration> {
        self.last_touched(id).map(|t| t.elapsed())
    }

    /// Like `age`, but relative to an explicitly given "now" — independent
    /// of the real clock at call time, for reproducible tests.
    pub fn age_at(&self, id: Id, now: Instant) -> Option<Duration> {
        self.last_touched(id).map(|t| now.saturating_duration_since(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_at_measures_explicit_duration_not_real_clock() {
        let t0 = Instant::now();
        let mut log: RealTouchLog<&str> = RealTouchLog::new();
        log.touch_at("a", t0);

        let five_seconds_later = t0 + Duration::from_secs(5);
        assert_eq!(log.age_at("a", five_seconds_later), Some(Duration::from_secs(5)));
    }

    #[test]
    fn age_is_none_for_id_never_touched() {
        let log: RealTouchLog<&str> = RealTouchLog::new();
        assert_eq!(log.age("ghost"), None);
    }

    #[test]
    fn touching_one_id_does_not_affect_another() {
        let t0 = Instant::now();
        let mut log: RealTouchLog<&str> = RealTouchLog::new();
        log.touch_at("a", t0);
        log.touch_at("b", t0 + Duration::from_secs(10));

        let now = t0 + Duration::from_secs(10);
        assert_eq!(log.age_at("a", now), Some(Duration::from_secs(10)), "touching 'b' must not change 'a's last_touched");
        assert_eq!(log.age_at("b", now), Some(Duration::ZERO));
    }

    #[test]
    fn real_touch_and_live_age_agree_approximately() {
        // A real-clock check (not deterministic in exact value, but
        // deterministic in direction and order of magnitude) — age_at
        // with an explicit Instant above covers the core logic; this
        // test only confirms that touch()/age() without an explicit
        // time actually work against real Instant::now() calls.
        let mut log: RealTouchLog<&str> = RealTouchLog::new();
        log.touch("a");
        std::thread::sleep(Duration::from_millis(5));
        let age = log.age("a").expect("should be Some after touch");
        assert!(age >= Duration::from_millis(5), "age must be at least the time actually slept");
        assert!(age < Duration::from_secs(5), "age must not explode after a short sleep");
    }
}
