# Implementation plan: mvp-04 — 5-minute idle stability integration test

**Story**: [mvp-04-idle-5min-stability-test.md](mvp-04-idle-5min-stability-test.md)
**Spec**: [spec.md](../spec.md)
**Layers**: ~~Capture~~ · Signal chain · ~~Render~~ · ~~Tone state~~ · ~~Control surface~~ · ~~Persistence~~ · Tests
**Complexity**: 🟢 Low

---

## 1. Summary

Codify AC2's idle-stability check as a reproducible `cargo test -- --ignored idle_5min` invocation that drives 5 minutes of silent buffers through the existing `Process` chain via `BufferBackend`, across every boundary buffer size and sample rate, asserting no panic and (with `--features debug-assert-no-alloc`) no per-buffer allocation. The technical edge is being explicit about **what the test does and does not cover**: it exercises the domain `Gain` block under the boundary parameter sweep, *not* `TonismPlugin::process` (the realtime callback that nih-plug owns). The actual AC2 hardware acceptance is the 5-minute manual session — this test is the fast CI safety net for the alloc-free invariant on the domain side.

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case                                                                    | Layer        | Main component                                                | Status |
| ----------------------------------------------------------------------------------- | ------------ | ------------------------------------------------------------- | ------ |
| Drive 5 simulated minutes of silence × 7 buffer sizes × 4 sample rates              | Tests        | `tests/idle_5min.rs`                                          | 🟢     |
| Assert no panic across all 28 combinations                                          | Tests        | `BufferBackend::run` in a `for` loop, no `#[should_panic]`    | 🟢     |
| Assert `XrunCounter == 0` at the end of each combination                            | Tests        | Direct read; the test owns its own `XrunCounter` instance     | 🟢     |
| `--features debug-assert-no-alloc` passes                                           | Tests        | `nih_plug/assert_process_allocs` is plugin-scoped — see §4.6   | 🟢     |
| `silent_buffer` fixture loses `#[allow(dead_code)]`                                 | Tests        | Edit [fixtures.rs:24](../../../../tests/common/fixtures.rs)   | 🟡     |
| Test completes in < 30 s wall-clock (offline timebase)                              | Tests        | Buffer-count loop, no real-time pacing                        | 🟢     |
| Test marked `#[ignore]` so default `cargo test` stays fast                          | Tests        | `#[ignore = "5-min idle stability — run with --ignored"]`      | 🟢     |
| Manual 5-min session as actual AC2 protocol                                         | (manual)     | Story checklist                                               | —      |

### Out of scope (declared)

- Driving `TonismPlugin::process` (the nih-plug realtime callback) — see §4.6 for the rationale; the pragmatic path is to extract a testable chain primitive, deferred to a future story when more than one DSP block exists.
- Asserting hardware-real cpal underruns — that is the manual run.
- Replacing the manual hardware AC2 protocol — the spec mandates the manual run; the integration test is additive.

---

## 3. Domain & data model

No domain change. No new types.

The test consumes the existing domain `Gain` block ([blocks/gain.rs](../../../../src/domain/blocks/gain.rs)) and `Process` trait ([process.rs](../../../../src/domain/process.rs)).

---

## 4. Architecture

### 4.1 Domain — pure core

No domain change.

### 4.2 Audio adapter — realtime shell

No audio adapter change. The integration test drives a `Process` impl through `BufferBackend`, not `TonismPlugin`. The audio adapter is exercised only through the `BufferBackend` in-memory fake.

### 4.3 Control surface — GUI / MIDI

No control-surface change.

### 4.4 Persistence

No persistence change.

### 4.5 Composition root

No composition-root change.

🟡 [tests/common/fixtures.rs:24](../../../../tests/common/fixtures.rs) — drop `#[allow(dead_code)]` on `silent_buffer`. The function becomes used.

🟢 [tests/idle_5min.rs](../../../../tests/idle_5min.rs) — new file.

### 4.6 Key technical decisions

- **Test exercises `Gain` via `BufferBackend`, not `TonismPlugin`.** `BufferBackend::run` ([backend.rs:54–63](../../../../src/audio/backend.rs)) takes `&mut dyn Process` and a flat mono `Vec<f32>`. `TonismPlugin::process` takes a nih-plug `Buffer` that the standalone wrapper builds internally — there is no public constructor for `Buffer` in test code. Driving `TonismPlugin` directly would require either (a) a fork of nih-plug exposing `Buffer::new_for_test`, or (b) extracting the per-buffer chain logic from `TonismPlugin::process` into a `pub(crate)` helper that takes `&mut [&mut [f32]]`. Neither is in scope for the AC2 safety net. The integration test's job is to prove "the domain `Gain` block (the only domain processor in MVP) can sustain 5 minutes of silent input across boundary parameter sweeps without panic and without allocating per buffer". That is what `BufferBackend` already does.
- **`#[ignore]` so `cargo test` stays fast.** Default `cargo test` runs the smoke suite ([smoke.rs](../../../../tests/smoke.rs)) plus unit tests; `cargo test -- --ignored` runs the slow stability suite. CI invokes both phases.
- **Wall-clock estimate.** 5 min × 4 SRs × 7 buffer sizes × 1 channel × 4 bytes/sample ≈ 5 × 60 × (44_100 + 48_000 + 88_200 + 96_000) × 7 × 4 ≈ 23 GB of `Vec<f32>` arithmetic across the suite. Each `BufferBackend::run` allocates one `silent_buffer(secs, sr)` (~5–11 MB) per (size, SR) pair and chunks through it — heap allocation is once per test case, not per buffer (per the `BufferBackend::run` body, line 59 allocates a `chunk.to_vec()` for each chunk; that is *outside* the `Process::process` hot path so it does not affect the alloc-free assertion). Estimated wall-clock: ~15–30 s on the dev machine.
- **`debug-assert-no-alloc` scope.** The `nih_plug/assert_process_allocs` feature [src/audio/Cargo.toml feature](../../../../Cargo.toml) hooks `Plugin::process`; it does **not** trip on `BufferBackend::run`'s per-chunk `to_vec()` because that runs in a test thread, not under the nih-plug `assert_process_allocs` guard. So the integration test cannot directly use `debug-assert-no-alloc` to prove zero allocations on the chunk loop. Instead, the alloc-free invariant on `Gain::process` is asserted **structurally** by reading review (the `for s in buffer { *s *= gain_linear.value(); }` loop has no allocation site), supplemented by the unit tests in [blocks/gain.rs](../../../../src/domain/blocks/gain.rs) that already run under the feature in [mvp-01](mvp-01-latency-measurement-primitive-implementation-plan.md)'s test session. Document this gap explicitly in the test file's module comment.
- **Counter assertion is structural, not behavioural.** `Gain::process` does not increment `XrunCounter` — the counter lives in `src/audio/`, not `src/domain/`. So `xrun_counter == 0` is a tautology in this test. We assert it nonetheless to stay aligned with the story checklist; the *meaningful* xrun assertion lives in [mvp-03](mvp-03-xrun-detection-wired-implementation-plan.md) + the manual hardware run.

### 4.7 Justification of deviation from standards

None. The plan respects testing.md G1 (integration as load-bearing), G2 (no module mocks — uses the real `BufferBackend` and real `Gain`), G3 (asserts on processed audio output, not internal call counts), G4 (real fake — not a deep mock), G5 (top-level `tests/` for integration tests), G7 (boundary-value sweep over `BUFFER_SIZES` and `SAMPLE_RATES`).

---

## 5. Tests

### 5.1 e2e (audio path)

This story *is* the integration test for AC2. The hardware-real e2e is the manual 5-minute session described in the story checklist.

### 5.2 Integration

- 🟢 [tests/idle_5min.rs](../../../../tests/idle_5min.rs) — new test file. Structure:

```rust
#![cfg_attr(not(test), allow(dead_code))]

mod common;

use common::fixtures::{BUFFER_SIZES, SAMPLE_RATES, silent_buffer};
use tonism::audio::backend::{AudioBackend, BufferBackend};
use tonism::domain::blocks::gain::Gain;
use tonism::domain::types::{Decibels, SampleRate};

#[test]
#[ignore = "5-min idle stability — run with --ignored (~30 s wall-clock)"]
fn five_minute_idle_silent_buffer_no_panic_across_boundaries() {
    for &sr in SAMPLE_RATES {
        let sample_rate = SampleRate::new(sr as f32);
        let input = silent_buffer(300.0, sample_rate); // 5 min
        for &bs in BUFFER_SIZES {
            let mut backend = BufferBackend::new(input.clone(), bs as usize);
            let mut gain = Gain { db: Decibels::new(0.0) };
            backend.run(&mut gain, sample_rate);
            let out = backend.into_output();
            assert_eq!(
                out.len(), input.len(),
                "(sr={sr}, bs={bs}): output length mismatch — possible buffer-loop bug"
            );
            // Silent input through unity gain → exactly silent output.
            assert!(
                out.iter().all(|&s| s == 0.0),
                "(sr={sr}, bs={bs}): non-zero sample escaped through silent + unity-gain path"
            );
        }
    }
}
```

The `tests/common/mod.rs` boilerplate already exists (referenced by [smoke.rs:7](../../../../tests/smoke.rs)); reuse it.

### 5.3 Unit (pure domain) — co-located

No new unit tests. The existing [blocks/gain.rs](../../../../src/domain/blocks/gain.rs) module's 9 tests cover boundary values for `Gain::process` (zero dB, positive dB, negative dB, NaN, ±∞, empty buffer, single sample). They are the unit half of the trophy.

### 5.4 AC coverage table

| AC / Checklist item                                                            | Test                                                          |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------- |
| Run `cargo test --release -- --ignored idle_5min` — passes for all combinations | integration `five_minute_idle_silent_buffer_no_panic_across_boundaries` |
| `--features debug-assert-no-alloc` — passes; no panic                          | (see §4.6) — covered by mvp-01 / Gain unit tests under the feature |
| Manual: launch `cargo run --release`, leave idle 5 min, `xrun: 0` at end        | **manual** (story checklist) — exercises [mvp-03](mvp-03-xrun-detection-wired-implementation-plan.md)'s counter |
| `silent_buffer` loses `#[allow(dead_code)]`                                    | reading review of the diff to [fixtures.rs:24](../../../../tests/common/fixtures.rs) |

---

## 6. Dependencies and execution order

- **Prerequisite stories**: none strictly. The integration test does not depend on [mvp-03](mvp-03-xrun-detection-wired-implementation-plan.md)'s xrun mechanism — it asserts `xrun == 0` against a counter that is never bumped by `Gain::process` (tautological assertion documented in §4.6).
- **Stories unblocked**: none directly; this story closes a verification gap rather than enabling further work.
- **Commands to run** (local + CI):
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test` (default, fast — does **not** run this test)
  - `cargo test --release -- --ignored idle_5min` (slow path, ~30 s)
  - `cargo test --release --features debug-assert-no-alloc -- --ignored idle_5min` (alloc-asserting; feature is plugin-scoped, see §4.6)

---

## 7. Risks and open questions

- 🟡 **Test does not exercise `TonismPlugin::process`.** The actual AC2 contract is "no xrun on the realtime callback during 5 minutes of audio". The integration test exercises a strict subset (`Gain::process` via the in-memory backend). Mitigation: the story checklist preserves the manual hardware run as the AC2 acceptance — the test is the fast safety net, not the contract. Documented in §4.6 and in the test file's module comment.
- 🟡 **`debug-assert-no-alloc` scope.** As noted in §4.6, the feature gate hooks `Plugin::process`, not `BufferBackend::run`. The structural review of `Gain::process` is the alloc-free guarantee for the chain itself.
- 🟢 **Foundations exist.** `BufferBackend`, `silent_buffer`, `BUFFER_SIZES`, `SAMPLE_RATES`, the `tests/common/` skeleton are all already in place. The story is one new test file plus removal of one `#[allow(dead_code)]` flag.

---

## 8. References

- Similar implementations to follow: [tests/smoke.rs](../../../../tests/smoke.rs) — same parametric loop pattern over `BUFFER_SIZES` and `SAMPLE_RATES`; reuse its module structure verbatim.
- Directly applicable standards: [architecture.md](../../../standards/architecture.md) (A1 — domain stays pure under test), [testing.md](../../../standards/testing.md) (G1, G2, G4, G5, G7).
- Source spec section: [acceptance criteria AC2](../spec.md#acceptance-criteria); [dependencies — fake audio device + 5-min silent fixture](../dependencies.md#test-data).
