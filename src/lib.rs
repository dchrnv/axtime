//! axtime: generic tick-based and wall-clock time tracking, independent
//! of any specific domain (no tokens, no particular data store, nothing
//! domain-specific inside). A system that has no real clock of its own
//! still needs *some* notion of time — so this crate offers a small menu
//! of independent "views" of time, and lets the caller pick what fits.
//!
//! Tick-based views (the primary [`Time`] facade):
//!
//! 1. **Ordinal** (Aristotle; McTaggart's B-series) — the tick number
//!    itself, stamped at the boundary between ticks. [`stamp`][Time::stamp].
//! 2. **Relative "now"** (McTaggart's A-series) — age = now − last_touched,
//!    computed on the fly, never stored. [`age`][Time::age].
//! 3. **Internal/durational** (Bergson) — age measured in accumulated
//!    growth, not in raw tick count; can stay small even after many ticks
//!    if growth was slow during them. [`age_inner`][Time::age_inner].
//! 5. **Cyclical** (Plato) — average period between an id's occurrences.
//!    [`period`][Time::period].
//!
//! Plus an independent [`wall`] module for real (calendar) time —
//! "duration in absolute units" — for the cases where wall-clock time is
//! genuinely useful (logs, timeouts, correlating with the outside world),
//! without displacing tick-based time as the primary notion.
//!
//! Deliberately out of scope: relativistic, branching/quantum, closed
//! time loops, and metaphysical notions of time. These are speculative
//! territory for future AI architectures in general, not something that
//! maps onto real, testable code today. Causality/arrow-of-time needs no
//! separate API either — it's already structurally guaranteed: `stamp`
//! only ever increases, and there is no method on this crate that could
//! decrease it.
//!
//! The caller drives the clock: call [`Time::advance`] once per tick and
//! [`Time::touch`] whenever an id is genuinely touched on that tick. This
//! crate has no opinion on what a "tick" represents in your domain.

pub mod wall;

use std::collections::HashMap;
use std::hash::Hash;

pub type Tick = u32;

/// The last tick on which an id was genuinely "touched". Separate from
/// mere presence — the caller decides what counts as a touch.
#[derive(Default)]
pub struct TouchLog<Id> {
    last_touched: HashMap<Id, Tick>,
}

impl<Id: Eq + Hash + Copy> TouchLog<Id> {
    pub fn new() -> Self {
        Self { last_touched: HashMap::new() }
    }

    pub fn touch(&mut self, id: Id, now: Tick) {
        self.last_touched.insert(id, now);
    }

    pub fn last_touched(&self, id: Id) -> Option<Tick> {
        self.last_touched.get(&id).copied()
    }

    /// Age (view 2) = `now - last_touched`. `None` if the id was never
    /// touched.
    pub fn age(&self, id: Id, now: Tick) -> Option<Tick> {
        self.last_touched(id).map(|last| now.saturating_sub(last))
    }
}

/// An accumulated growth series, one entry per tick (view 3: internal
/// time — Bergson's duration is measured in accumulated growth, not in
/// the raw number of ticks).
#[derive(Default)]
pub struct GrowthSeries {
    inner: Vec<f64>,
}

impl GrowthSeries {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Append this tick's growth — accumulates on top of the previous
    /// value, does not replace it.
    pub fn push(&mut self, delta: f64) {
        let last = self.inner.last().copied().unwrap_or(0.0);
        self.inner.push(last + delta);
    }

    /// The accumulated value at the end of `tick`. `inner[0]` in storage
    /// corresponds to tick 1 (the first `push`), so tick 0 (before any
    /// `advance`) is the 0.0 baseline, not a stored element.
    pub fn at(&self, tick: Tick) -> Option<f64> {
        if tick == 0 {
            return Some(0.0);
        }
        self.inner.get((tick - 1) as usize).copied()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// `inner[now] - inner[last_touched]` — internal age.
    pub fn age_inner(&self, now: Tick, last_touched: Tick) -> Option<f64> {
        let now_v = self.at(now)?;
        let then_v = self.at(last_touched)?;
        Some(now_v - then_v)
    }
}

/// History of ticks on which an id was genuinely encountered — backs
/// the period view (view 5).
#[derive(Default)]
pub struct OccurrenceLog<Id> {
    history: HashMap<Id, Vec<Tick>>,
}

impl<Id: Eq + Hash + Copy> OccurrenceLog<Id> {
    pub fn new() -> Self {
        Self { history: HashMap::new() }
    }

    pub fn record(&mut self, id: Id, tick: Tick) {
        self.history.entry(id).or_default().push(tick);
    }

    pub fn occurrences(&self, id: Id) -> &[Tick] {
        self.history.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Period (view 5) — the average gap between consecutive
    /// occurrences. `None` if there are fewer than two occurrences (no
    /// period can be derived).
    pub fn period(&self, id: Id) -> Option<f64> {
        let h = self.history.get(&id)?;
        if h.len() < 2 {
            return None;
        }
        let gaps_sum: u32 = h.windows(2).map(|w| w[1] - w[0]).sum();
        Some(gaps_sum as f64 / (h.len() - 1) as f64)
    }
}

/// A facade over the three logs above — the minimal API
/// (`stamp`/`age`/`age_inner`/`period`); one tick equals one call to
/// `advance`.
#[derive(Default)]
pub struct Time<Id> {
    now: Tick,
    touches: TouchLog<Id>,
    growth: GrowthSeries,
    occurrences: OccurrenceLog<Id>,
}

impl<Id: Eq + Hash + Copy> Time<Id> {
    pub fn new() -> Self {
        Self {
            now: 0,
            touches: TouchLog::new(),
            growth: GrowthSeries::new(),
            occurrences: OccurrenceLog::new(),
        }
    }

    /// Advance one tick. `growth_delta` is this tick's growth (e.g. this
    /// cycle's vocabulary growth in a token engine) — appended to the
    /// accumulated series before any `touch` calls for this tick.
    pub fn advance(&mut self, growth_delta: f64) {
        self.now += 1;
        self.growth.push(growth_delta);
    }

    /// A genuine touch of `id` on the current tick — updates both
    /// last_touched and the occurrence history at once (one event, not
    /// two independent ones).
    pub fn touch(&mut self, id: Id) {
        self.touches.touch(id, self.now);
        self.occurrences.record(id, self.now);
    }

    /// View 1: ordinal time — the current tick number.
    pub fn stamp(&self) -> Tick {
        self.now
    }

    /// View 2: age in ticks since the last touch.
    pub fn age(&self, id: Id) -> Option<Tick> {
        self.touches.age(id, self.now)
    }

    /// View 3: age in accumulated growth since the last touch.
    pub fn age_inner(&self, id: Id) -> Option<f64> {
        let last = self.touches.last_touched(id)?;
        self.growth.age_inner(self.now, last)
    }

    /// View 5: average period between this id's occurrences, in ticks.
    pub fn period(&self, id: Id) -> Option<f64> {
        self.occurrences.period(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_matches_number_of_advances() {
        let mut t: Time<u32> = Time::new();
        assert_eq!(t.stamp(), 0);
        t.advance(0.0);
        t.advance(0.0);
        assert_eq!(t.stamp(), 2);
    }

    #[test]
    fn age_is_zero_right_after_touch_and_grows_afterwards() {
        let mut t: Time<&str> = Time::new();
        t.advance(0.0);
        t.touch("a");
        assert_eq!(t.age("a"), Some(0));
        t.advance(0.0);
        t.advance(0.0);
        assert_eq!(t.age("a"), Some(2));
    }

    #[test]
    fn age_is_none_for_id_never_touched() {
        let t: Time<&str> = Time::new();
        assert_eq!(t.age("ghost"), None);
    }

    #[test]
    fn stale_id_sitting_untouched_does_not_get_age_reset_by_other_touches() {
        let mut t: Time<&str> = Time::new();
        t.advance(0.0);
        t.touch("a");
        t.advance(0.0);
        t.touch("b"); // touch a different id — "a" must not be affected
        assert_eq!(t.age("a"), Some(1), "touching another id must not reset 'a's age");
    }

    #[test]
    fn age_inner_tracks_accumulated_growth_not_raw_ticks() {
        let mut t: Time<&str> = Time::new();
        t.advance(10.0); // tick 1, inner = 10
        t.touch("a");
        t.advance(1.0); // tick 2, inner = 11 (barely grows)
        t.advance(1.0); // tick 3, inner = 12
        // age in ticks = 2, and age_inner = 12 - 10 = 2.0 — numerically
        // equal here; check a case with a different growth rate instead:
        let mut t2: Time<&str> = Time::new();
        t2.advance(100.0); // tick 1, inner = 100
        t2.touch("a");
        t2.advance(0.1); // tick 2, inner = 100.1
        t2.advance(0.1); // tick 3, inner = 100.2
        assert_eq!(t2.age("a"), Some(2), "age in ticks is always a whole number of ticks");
        let inner = t2.age_inner("a").unwrap();
        assert!((inner - 0.2).abs() < 1e-9, "internal age stays small despite 2 ticks (growth rate dropped)");
    }

    #[test]
    fn period_is_none_with_fewer_than_two_occurrences() {
        let mut t: Time<&str> = Time::new();
        t.advance(0.0);
        t.touch("a");
        assert_eq!(t.period("a"), None);
    }

    #[test]
    fn period_is_average_gap_between_occurrences() {
        let mut t: Time<&str> = Time::new();
        for _ in 0..3 {
            t.advance(0.0);
        }
        t.touch("a"); // tick 3
        for _ in 0..3 {
            t.advance(0.0);
        }
        t.touch("a"); // tick 6, gap 3
        for _ in 0..5 {
            t.advance(0.0);
        }
        t.touch("a"); // tick 11, gap 5
        assert_eq!(t.period("a"), Some(4.0), "average gap (3+5)/2 = 4.0");
    }
}
