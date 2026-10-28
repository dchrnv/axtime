# axtime

Generic tick-based and wall-clock time tracking for systems that don't have — or don't want — a built-in notion of real time.

No dependencies. No domain assumptions. Generic over any `Id: Eq + Hash + Copy`.

## Why

A system with no real clock of its own still needs *some* notion of time. This crate offers a small menu of independent views, so the caller picks what fits instead of being handed one hard-coded notion of "time":

1. **Ordinal** — the tick number itself (`stamp`).
2. **Relative age** — `now - last_touched`, computed on the fly (`age`).
3. **Internal/durational** — age measured in accumulated growth rather than raw tick count; stays small if growth was slow even across many ticks (`age_inner`).
5. **Cyclical** — the average period between an id's occurrences (`period`).

Plus an independent [`wall`] module for real (calendar/wall-clock) time, for the cases where that's genuinely useful — logs, timeouts, correlating with the outside world — without displacing tick-based time as the primary notion.

Deliberately out of scope: relativistic, branching/quantum, closed time-loop, and metaphysical notions of time. Speculative territory for future architectures in general, not something that maps onto real, testable code today.

## Usage

```rust
use axtime::Time;

let mut t: Time<&str> = Time::new();

t.advance(0.0);      // one tick, no growth this time
t.touch("a");         // "a" is touched on this tick
assert_eq!(t.age("a"), Some(0));

t.advance(5.0);       // another tick, with some growth
assert_eq!(t.age("a"), Some(1));
assert_eq!(t.age_inner("a"), Some(5.0));
```

```rust
use axtime::wall::RealTouchLog;

let mut log: RealTouchLog<&str> = RealTouchLog::new();
log.touch("a");
// ... later ...
let elapsed = log.age("a").unwrap(); // a real std::time::Duration
```

## Design notes

`advance()`/`touch()` are driven entirely by the caller — this crate has no opinion on what a "tick" represents in your domain (a byte-stream revolution, a game turn, a clock minute, anything discrete and orderable).

The caller also decides what counts as a "touch" — this crate does not infer it from mere presence in some other data structure.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
