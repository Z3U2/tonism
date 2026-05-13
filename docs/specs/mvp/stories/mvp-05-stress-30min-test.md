# mvp-05 — 30-minute stress test with deterministic parameter sweep

**As a** developer running the stress suite, **in** a terminal at the repo root, **when** running `cargo test --release --features debug-assert-no-alloc -- --ignored stress_30min`, **then** the test drives 30 simulated minutes of buffers through the `Process` chain with a deterministic parameter schedule (gain ramps and bypass toggles) and asserts no panic, zero allocations per buffer, and the xrun count stays within a documented warm-up tolerance.

> Derived from spec: [MVP — Glitch-free real-time guitar signal path](../spec.md)

## Functional description

Codify AC3's stress check as a reproducible integration test using the schedule from `dependencies.md`: a gain ramp every 2 simulated seconds, a bypass toggle every 5 simulated seconds, sustained over 30 simulated minutes. Driven through `BufferBackend` so the wall-clock cost is dominated by buffer arithmetic, not real time. ⚠️ TR required to confirm whether the timebase is compressed-via-`BufferBackend` (this story) or real-wall-clock (kept manual). Parameter changes are written through the same nih-plug params + smoothers the GUI uses, so what the test exercises matches what the user does at runtime. The audio callback under exercise stays alloc/lock/syscall-free per A2 — `--features debug-assert-no-alloc` enforces it. Complements but does not replace the manual 30-minute standalone run in the spec's verification protocol.

Layers touched: Capture + Signal chain + Tone state.

## Acceptance criteria

### Success scenarios

- Running the test feeds 30 simulated minutes of buffers through the chain with the deterministic schedule applied, finishes without panicking, and asserts `XrunCounter::value()` stays within the documented warm-up tolerance (typically `0`, possibly `≤ N` if a known startup event is tolerated).
- With `--features debug-assert-no-alloc`, the test passes — proving no allocation per buffer even with parameter changes ramping continuously.
- The test runs at a representative subset of `BUFFER_SIZES` × `SAMPLE_RATES` (TR to confirm exact subset; full sweep would balloon CI cost).

### Failure scenarios

- A bypass toggle that allocates, locks, or contends with the smoother triggers an alloc-feature panic and the test fails at the offending toggle.
- A gain change that produces an audible discontinuity larger than the smoother window — visible as a hard step in the produced sample buffer — fails the test against an amplitude-delta assertion.
- If `XrunCounter` climbs beyond the documented tolerance, the test fails and reports the simulated time at which it climbed.

## Manual validation checklist

- [ ] Run `cargo test --release -- --ignored stress_30min` — passes within the wall-clock budget the TR sets.
- [ ] Run `cargo test --release --features debug-assert-no-alloc -- --ignored stress_30min` — passes; no allocation panic over the full 30-min schedule.
- [ ] As a separate manual check (the actual AC3 protocol), launch `cargo run --release`, run a 30-minute session, vary input gain, output gain, and bypass throughout, confirm no crash and `xrun:` does not climb beyond what AC2 already validated.
- [ ] Confirm the test's deterministic schedule is documented in code comments referencing the `dependencies.md` source so future changes to the schedule are traceable.
