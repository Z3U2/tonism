# mvp-01 — Latency measurement primitive

**As a** developer running the domain test suite, **in** a terminal at the repo root, **when** running `cargo test latency`, **then** the round-trip latency primitive recovers known synthetic delays from a Kronecker impulse and its loopback within 1 sample, across all boundary sample rates.

> Derived from spec: [MVP — Glitch-free real-time guitar signal path](../spec.md)

## Functional description

Add a pure-domain function that computes the round-trip delay (in samples and milliseconds) between a reference impulse and its observed loopback by cross-correlation, given the session sample rate. This is the math that AC1 hinges on; mvp-02 will drive it from the GUI with a real captured loopback. The function lives in `src/domain/` next to the existing types and `Process` trait, takes only `&[f32]` plus a `SampleRate` NewType, and returns a `Result` that surfaces the "no usable peak" failure as a domain error rather than a panic. No I/O, no allocation on the hot read path.

Layers touched: Signal chain.

## Acceptance criteria

### Success scenarios

- A 1024-sample Kronecker impulse plus a synthetic loopback shifted by N samples (0, 32, 256, 2048) yields a measured delay equal to N at sample rates 44 100, 48 000, 88 200, 96 000 Hz, with the corresponding ms reading rounded to one decimal place.
- The function operates on borrowed slices and does not allocate inside its hot computation (verifiable with `--features debug-assert-no-alloc` on a test that calls it).

### Failure scenarios

- A loopback that is silence (no detectable peak above a documented noise floor) returns a domain `Error` variant — not a panic, not a `0` masquerading as a valid result.
- A reference impulse and loopback of incompatible lengths (loopback shorter than impulse) returns an `Error` rather than producing an out-of-bounds read.
- A `SampleRate(0)` argument is rejected at the boundary (the existing `InvalidSampleRate` path applies).

## Manual validation checklist

- [ ] Run `cargo test latency` from the repo root — test names referencing the new primitive pass.
- [ ] Run `cargo test --features debug-assert-no-alloc latency` — passes with no allocation panic.
- [ ] Inspect a failing-case test (silent loopback) — assertion confirms the `Result::Err` variant, no panic.
- [ ] Run `cargo clippy --all-targets -- -D warnings` — no new warnings introduced by the primitive.
