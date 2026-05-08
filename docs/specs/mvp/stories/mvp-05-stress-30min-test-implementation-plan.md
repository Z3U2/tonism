# Implementation plan: mvp-05 — 30-minute stress test with deterministic parameter sweep

**Story**: [mvp-05-stress-30min-test.md](mvp-05-stress-30min-test.md)
**Spec**: [spec.md](../spec.md)
**Layers**: ~~Capture~~ · Signal chain · ~~Render~~ · Tone state · ~~Control surface~~ · ~~Persistence~~ · Tests
**Complexity**: 🟡 Medium

---

## 1. Summary

Codify AC3's stress check as a reproducible `cargo test --release -- --ignored stress_30min` invocation that drives 30 simulated minutes of buffers through the domain `Gain` block while applying the deterministic parameter schedule from [dependencies.md](../dependencies.md#test-data) — a `Gain.db` ramp every 2 simulated seconds and a bypass-equivalent toggle every 5 simulated seconds — asserting no panic and no per-buffer allocation. Same scoping caveat as [mvp-04](mvp-04-idle-5min-stability-test-implementation-plan.md): the test exercises the domain processor under the parameter sweep, not the full `TonismPlugin` callback. The technical edge is choosing a driver loop that bypasses `BufferBackend::run`'s "prepare-once-then-loop" lifecycle so the test can mutate `Gain.db` *between* buffers without re-preparing each time.

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case                                                                        | Layer        | Main component                                                        | Status |
| --------------------------------------------------------------------------------------- | ------------ | --------------------------------------------------------------------- | ------ |
| 30 simulated minutes through the chain with deterministic schedule, no panic            | Tests        | `tests/stress_30min.rs` driver loop                                   | 🟢     |
| Gain ramps every 2 simulated seconds                                                    | Tests        | `next_db(elapsed_seconds)` schedule helper                            | 🟢     |
| Bypass-equivalent toggle every 5 simulated seconds                                      | Tests        | `is_bypassed(elapsed_seconds)` schedule helper                        | 🟢     |
| Schedule applied via the same parameter type the GUI uses (`Decibels`)                  | Tests        | Reuses `Decibels::new` ([types.rs:43](../../../../src/domain/types.rs)) | ⚪     |
| Test runs at compressed speed (offline) on a representative SR × buffer-size subset     | Tests        | Subset chosen in §4.6                                                 | 🟢     |
| Test passes with `--features debug-assert-no-alloc` (see §4.6 scope caveat)             | Tests        | (per scope caveat — see [mvp-04](mvp-04-idle-5min-stability-test-implementation-plan.md) §4.6) | 🟢     |
| `XrunCounter` stays within documented warm-up tolerance                                 | Tests        | Test owns its own `XrunCounter` (Gain doesn't bump it) → tautology     | 🟢     |
| Test marked `#[ignore]` so default `cargo test` stays fast                              | Tests        | `#[ignore = "30-min stress — run with --ignored"]`                     | 🟢     |
| Schedule documented in code comments referencing `dependencies.md`                      | Tests        | Module-level rustdoc                                                  | 🟢     |

### Out of scope (declared)

- Driving `TonismPlugin::process` directly (same rationale as [mvp-04 §4.6](mvp-04-idle-5min-stability-test-implementation-plan.md) — `Buffer` has no test constructor). Manual hardware 30-min run remains the actual AC3 acceptance.
- Real-wall-clock 30-minute run in CI — compressed offline timebase only; the spec's manual protocol is the wall-clock check.
- Discontinuity assertions on the audio output. The story's failure scenario "audible discontinuity larger than the smoother window" assumes parameter smoothing. `Gain` is unsmoothed at the domain level — smoothing is done by nih-plug's `FloatParam` smoother *outside* the domain block. The integration test therefore asserts no panic + alloc-free + no NaN/Inf escape under ramping `db`, which is the strongest behavioural assertion possible without exercising `TonismPlugin`.

---

## 3. Domain & data model

No domain change. No new types. The test parametrises an existing `Gain` block by mutating its public `db: Decibels` field ([blocks/gain.rs:9](../../../../src/domain/blocks/gain.rs)).

---

## 4. Architecture

### 4.1 Domain — pure core

No domain change.

### 4.2 Audio adapter — realtime shell

No audio adapter change. (See §4.6 for why we cannot drive `TonismPlugin::process` directly in a unit / integration test.)

### 4.3 Control surface — GUI / MIDI

No control-surface change.

### 4.4 Persistence

No persistence change.

### 4.5 Composition root

🟢 [tests/stress_30min.rs](../../../../tests/stress_30min.rs) — new file.

🟡 [tests/common/fixtures.rs](../../../../tests/common/fixtures.rs) — `silent_buffer` is reused (drop `#[allow(dead_code)]` was already done by [mvp-04](mvp-04-idle-5min-stability-test-implementation-plan.md); confirm in review).

### 4.6 Key technical decisions

- **Driver loop bypasses `BufferBackend::run`.** `BufferBackend::run` ([backend.rs:54–63](../../../../src/audio/backend.rs)) calls `prepare → reset → loop chunks`; once the loop starts, the `Gain` is owned mutably by the backend and cannot be mutated externally. The stress test needs to mutate `gain.db` *between* buffers. Two options:
  - **(α)** Skip `BufferBackend` entirely. Allocate a 30-min silent input vec; iterate `chunks_mut` directly; mutate `gain.db` on the schedule; call `gain.process(chunk)` per chunk. Simpler; no abstraction lost (the test is exercising `Process`, not `AudioBackend`).
  - **(β)** Add `BufferBackend::run_with_param_callback` that invokes a closure between chunks. Reusable, but only this test would consume it.
  - **Recommended: α.** The driver loop is ~10 lines and is naturally a part of the test rather than a backend feature.
- **Subset of `BUFFER_SIZES` × `SAMPLE_RATES`.** Full sweep = 28 combinations × 30 min × ~96k samples/s ≈ minutes of wall-clock. Pick a representative subset:
  - `BUFFER_SIZES`: `64`, `256`, `2048` (small / typical / large).
  - `SAMPLE_RATES`: `48_000`, `96_000` (most common gigging-rig norm + high-rate stress).
  - 6 combinations, ~30 s wall-clock total estimated. Document the subset choice in the test rustdoc.
- **Schedule per `dependencies.md`**:
  - `next_db(elapsed_seconds)`: linear ramp from `-6.0 dB` to `+6.0 dB` over 2 s, repeating; sawtooth pattern. `((elapsed_seconds % 2.0) / 2.0) * 12.0 - 6.0`.
  - `is_bypassed(elapsed_seconds)`: toggles every 5 s — `((elapsed_seconds / 5.0).floor() as u64) % 2 == 1`.
  - When "bypassed", the test driver skips `gain.process(chunk)` (mirrors `TonismPlugin::process`'s bypass branch [plugin.rs:174–176](../../../../src/audio/plugin.rs)). The chunk passes through untouched.
- **Counter assertion is tautological** — same as [mvp-04 §4.6](mvp-04-idle-5min-stability-test-implementation-plan.md). `Gain::process` does not bump `XrunCounter`; the assertion `xrun_counter == 0` is structural, not behavioural. The meaningful xrun assertion is the manual hardware run + [mvp-03](mvp-03-xrun-detection-wired-implementation-plan.md)'s wired counter.
- **Alloc-asserting feature scope** — same caveat as mvp-04: `nih_plug/assert_process_allocs` is plugin-scoped. Structural review of `Gain::process` (which is one `for s in buffer { *s *= ...; }` loop, no alloc site) is the alloc-free guarantee for the chain.
- **Warm-up tolerance for the xrun assertion** — set to `0` since `Gain` cannot bump the counter. If a future story extends the integration test to drive `TonismPlugin`, this becomes a real number.

### 4.7 Justification of deviation from standards

None. The plan respects testing.md G1, G3 (asserts on output, not internal call counts), G4 (real `Gain`, real chunk loop — not mocks), G5 (`tests/` top-level), G7 (boundary subset across SR and buffer size).

---

## 5. Tests

### 5.1 e2e (audio path)

This story *is* the integration test for AC3. The hardware-real e2e is the manual 30-minute session described in the story checklist.

### 5.2 Integration

- 🟢 [tests/stress_30min.rs](../../../../tests/stress_30min.rs) — new test file. Structure:

```rust
//! Deterministic stress harness for AC3. Schedule sourced from
//! docs/specs/mvp/dependencies.md "Stress automation":
//!   - gain.db ramps every 2 simulated seconds (sawtooth -6..+6 dB)
//!   - bypass toggles every 5 simulated seconds (driver-side, mirrors
//!     TonismPlugin::process bypass branch)
//!
//! Subset chosen for CI wall-clock budget — see TR plan §4.6.
//! Manual 30-min hardware run remains the actual AC3 protocol.

mod common;

use common::fixtures::silent_buffer;
use tonism::domain::blocks::gain::Gain;
use tonism::domain::process::Process;
use tonism::domain::types::{Decibels, SampleRate};

const BUFFER_SIZES_SUBSET: &[u32] = &[64, 256, 2048];
const SAMPLE_RATES_SUBSET: &[u32] = &[48_000, 96_000];
const STRESS_DURATION_SECS: f32 = 30.0 * 60.0;

fn next_db(elapsed_seconds: f32) -> f32 {
    ((elapsed_seconds % 2.0) / 2.0) * 12.0 - 6.0
}

fn is_bypassed(elapsed_seconds: f32) -> bool {
    ((elapsed_seconds / 5.0).floor() as u64) % 2 == 1
}

#[test]
#[ignore = "30-min stress with parameter sweep — run with --ignored (~30 s wall-clock)"]
fn thirty_minute_stress_no_panic_across_subset() {
    for &sr in SAMPLE_RATES_SUBSET {
        let sample_rate = SampleRate::new(sr as f32);
        let mut input = silent_buffer(STRESS_DURATION_SECS, sample_rate);
        for &bs in BUFFER_SIZES_SUBSET {
            let mut gain = Gain { db: Decibels::new(0.0) };
            gain.prepare(sample_rate, bs as usize);
            gain.reset();
            let frames_per_second = sr as f32;
            for (chunk_index, chunk) in input.chunks_mut(bs as usize).enumerate() {
                let elapsed_seconds =
                    (chunk_index as f32 * bs as f32) / frames_per_second;
                gain.db = Decibels::new(next_db(elapsed_seconds));
                if !is_bypassed(elapsed_seconds) {
                    gain.process(chunk);
                }
                debug_assert!(
                    chunk.iter().all(|s| s.is_finite()),
                    "(sr={sr}, bs={bs}, chunk={chunk_index}): non-finite sample escaped"
                );
            }
        }
    }
}
```

The output is silent input → finite output (gain × 0 = 0 always); the `is_finite()` debug_assert is a defensive check against any future Gain stateful refactor that could leak NaN under rapid db changes.

### 5.3 Unit (pure domain) — co-located

- 🟢 `next_db_sawtooth_returns_minus_six_to_plus_six` — co-located in `tests/stress_30min.rs` (or in a `tests/stress_schedule.rs` companion module if the schedule helpers grow). Boundary values: `next_db(0.0) == -6.0`; `next_db(1.0) == 0.0`; `next_db(2.0 - eps) ≈ 6.0 - tiny`; `next_db(2.0) == -6.0` (period reset).
- 🟢 `is_bypassed_toggles_every_five_seconds` — `is_bypassed(0.0) == false`; `is_bypassed(4.99) == false`; `is_bypassed(5.0) == true`; `is_bypassed(9.99) == true`; `is_bypassed(10.0) == false`.

These keep the schedule maintainable; if the spec changes (every 3 s, every 7 s) the unit tests fail loudly.

### 5.4 AC coverage table

| AC / Checklist item                                                                       | Test                                                          |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| Run `cargo test --release -- --ignored stress_30min` — passes for the subset              | integration `thirty_minute_stress_no_panic_across_subset`     |
| `--features debug-assert-no-alloc` — passes; no panic                                     | (see §4.6) — covered by mvp-01 / Gain unit tests under feature |
| Manual: launch `cargo run --release`, run 30 min, vary controls, no crash, xrun stable    | **manual** (story checklist) — exercises mvp-03's counter      |
| Schedule documented in code referencing `dependencies.md`                                 | reading review of the file's module rustdoc                   |
| Gain ramps every 2 s                                                                      | unit `next_db_sawtooth_returns_minus_six_to_plus_six`         |
| Bypass toggles every 5 s                                                                  | unit `is_bypassed_toggles_every_five_seconds`                 |

---

## 6. Dependencies and execution order

- **Prerequisite stories**: [mvp-04](mvp-04-idle-5min-stability-test-implementation-plan.md) is recommended first (drops `#[allow(dead_code)]` on `silent_buffer` — this story expects it to be reachable). If mvp-04 has not landed, this story drops the flag itself.
- **Stories unblocked**: none — completes the AC3 verification artifact.
- **Commands to run** (local + CI):
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test stress_schedule` (the schedule unit tests run by default; fast)
  - `cargo test --release -- --ignored stress_30min` (slow path, ~30 s)
  - `cargo test --release --features debug-assert-no-alloc -- --ignored stress_30min` (alloc-asserting; per §4.6 scope caveat)

---

## 7. Risks and open questions

- 🟡 **Test does not exercise `TonismPlugin::process`.** Same caveat as mvp-04 §4.6. Mitigation: manual hardware 30-min run is the AC3 acceptance.
- 🟡 **Subset of boundaries chosen (6 of 28).** A pathological combination outside the subset could mask a bug; spec's verification protocol still mandates the manual hardware run which uses whatever buffer size / SR the dev machine uses.
- 🟡 **Schedule helpers may not match v0.2 expectations** — once a real signal chain (multi-block) lands, the stress schedule may want to exercise more parameters. The current schedule implements `dependencies.md`'s spec verbatim; expand later when v0.2 introduces more parameters.
- 🟢 **Foundations exist.** `silent_buffer`, `Gain`, `Process` lifecycle are all in place; the story is one new test file plus two helper functions.

---

## 8. References

- Similar implementations to follow: [tests/smoke.rs](../../../../tests/smoke.rs) for the boundary-loop pattern; `Gain::process` ([blocks/gain.rs:13–18](../../../../src/domain/blocks/gain.rs)) for the operation under test.
- Directly applicable standards: [architecture.md](../../../standards/architecture.md) (A1 — domain stays pure under test), [testing.md](../../../standards/testing.md) (G1, G3, G4, G5, G7), [infrastructure.md](../../../standards/infrastructure.md) (J1 — parameter-sweep stress is the operational guarantee for AC3 latency / xruns).
- Source spec section: [acceptance criteria AC3](../spec.md#acceptance-criteria); [dependencies — stress automation schedule](../dependencies.md#test-data); [mvp-04 plan §4.6](mvp-04-idle-5min-stability-test-implementation-plan.md) for shared scope rationale.
