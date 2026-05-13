# mvp-04 — 5-minute idle stability integration test

**As a** developer running the integration suite, **in** a terminal at the repo root, **when** running `cargo test --release --features debug-assert-no-alloc -- --ignored idle_5min`, **then** the test drives 5 minutes of silent buffers through the `Process` chain via `BufferBackend` at every boundary buffer size and sample rate, and asserts no panic, zero allocations on the per-buffer path, and `XrunCounter == 0`.

> Derived from spec: [MVP — Glitch-free real-time guitar signal path](../spec.md)

## Functional description

Codify AC2's idle-stability check as a reproducible integration test alongside `tests/smoke.rs`. The test reuses the existing `silent_buffer(secs, sr)` fixture (currently `#[allow(dead_code)]`), the `BUFFER_SIZES` and `SAMPLE_RATES` constants, the `BufferBackend`, and the same `Gain` block the standalone uses. It is marked `#[ignore]` so the regular `cargo test` run stays fast; CI and the verification protocol invoke it explicitly. The audio callback path under exercise must remain alloc-free (enforced by `--features debug-assert-no-alloc`). It complements — but does not replace — the manual 5-minute standalone session in the spec's verification protocol.

Layers touched: Capture + Signal chain.

## Acceptance criteria

### Success scenarios

- The test feeds 5 simulated minutes of silence at every combination of `BUFFER_SIZES` × `SAMPLE_RATES` through the `Gain` block via `BufferBackend`, completes without panicking, and asserts `XrunCounter::value() == 0` at the end of each combination.
- With `--features debug-assert-no-alloc`, the test passes — confirming no allocation on the per-buffer path even after 5 minutes of buffers.
- Running the test alone (`-- --ignored idle_5min`) completes in a reasonable wall-clock window (< ~30 s when running offline through `BufferBackend`, since the timebase is the buffer count, not real time).

### Failure scenarios

- If the `Gain` block or any dependency starts allocating per buffer, the alloc feature triggers a panic and the test fails with a clear allocation-site stack.
- If the xrun mechanism wired by mvp-03 reports a non-zero count during the silent run, the test fails with the offending (sample rate, buffer size) pair.
- If a boundary buffer size + sample rate combination is not yet supported by the chain, the test fails with a domain `Error` rather than a generic panic.

## Manual validation checklist

- [ ] Run `cargo test --release -- --ignored idle_5min` — passes for all `BUFFER_SIZES` × `SAMPLE_RATES`.
- [ ] Run `cargo test --release --features debug-assert-no-alloc -- --ignored idle_5min` — passes; no allocation panic.
- [ ] As a separate manual check (the actual AC2 protocol), launch `cargo run --release`, leave the standalone idle for 5 minutes, observe `xrun: 0` at the end.
- [ ] Confirm the previously-dead `silent_buffer` fixture loses its `#[allow(dead_code)]` annotation (it is now used).
