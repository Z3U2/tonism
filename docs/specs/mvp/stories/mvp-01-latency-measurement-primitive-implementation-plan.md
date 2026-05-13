# Implementation plan: mvp-01 — Latency measurement primitive

**Story**: [mvp-01-latency-measurement-primitive.md](mvp-01-latency-measurement-primitive.md)
**Spec**: [spec.md](../spec.md)
**Layers**: ~~Capture~~ · Signal chain · ~~Render~~ · ~~Tone state~~ · ~~Control surface~~ · ~~Persistence~~ · Tests
**Complexity**: 🟢 Low

---

## 1. Summary

Add a pure-domain function that recovers the round-trip delay between a reference impulse and a captured loopback by O(N·M) cross-correlation, returning a `LatencyMs` NewType wrapped in a `Result`. The technical edge is keeping the kernel allocation-free on borrowed slices so mvp-02 can call it from anywhere without a heap path, and surfacing the silence/length-mismatch failures as `DomainError` variants instead of panics.

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case                                                        | Layer        | Main component                                        | Status |
| ----------------------------------------------------------------------- | ------------ | ----------------------------------------------------- | ------ |
| Kronecker impulse + N-shifted loopback → measured delay = N (N ∈ {0, 32, 256, 2048}) | Signal chain | `domain::latency::measure_latency`                    | 🟢     |
| Match holds at SR ∈ {44_100, 48_000, 88_200, 96_000} Hz                 | Signal chain | `LatencyMs` ms conversion via `SampleRate::value()`   | 🟢     |
| ms reading rounded to 1 decimal place                                   | Signal chain | `LatencyMs::new` rounding helper                      | 🟢     |
| Operates on borrowed slices, no per-call alloc on the kernel            | Signal chain | `measure_latency` body (slice-only)                   | 🟢     |
| Silent loopback → `Err(DomainError::LatencyNoPeak)`                     | Signal chain | Peak-threshold branch                                 | 🟢     |
| Loopback shorter than impulse → `Err(DomainError::LoopbackTooShort)`    | Signal chain | Length-precondition branch                            | 🟢     |
| `SampleRate::new(0.0)` → `Err(DomainError::InvalidSampleRate(0))`       | Signal chain | Sample-rate precondition (validates against existing variant) | 🟢     |

**Manual checklist**:
- "`cargo test latency`" → unit tests in `src/domain/latency.rs`.
- "`cargo test --features debug-assert-no-alloc latency`" → same tests under the alloc-asserting feature; `nih_plug/assert_process_allocs` only fires inside `Plugin::process`, so for a domain-only module we lean on the borrow-only kernel + a manual reading review.
- "Failing-case test confirms `Err`, no panic" → silence and length-mismatch unit tests assert `assert!(matches!(result, Err(DomainError::...)))`.
- "`cargo clippy --all-targets -- -D warnings`" → enforced by CI + lefthook ([`lefthook.yml`](../../../../lefthook.yml)).

### Out of scope (declared)

- Driving the primitive from the audio adapter or the GUI — that is **mvp-02**.
- Measuring on a real audio device — also mvp-02 + the manual verification protocol.
- Fancy peak-finder (parabolic interpolation, sub-sample precision) — the AC bar is "within 1 sample" and an integer argmax meets it.

---

## 3. Domain & data model

### New / modified types

| Change                                                  | Description                                                                                                     | Status |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ------ |
| 🟢 NewType `LatencyMs(f32)`                             | One-decimal-rounded ms value (rule C5: private field, `pub const fn new(f32) -> Self`, `pub fn value(self) -> f32`) | 🟢     |
| 🟡 `DomainError` (extend)                               | Add `LatencyNoPeak` and `LoopbackTooShort` variants; reuse existing `InvalidSampleRate(u32)` for SR-zero        | 🟡     |
| 🟢 Free function `measure_latency`                      | `fn measure_latency(reference: &[f32], loopback: &[f32], sr: SampleRate) -> Result<LatencyMs, DomainError>`     | 🟢     |

The existing `DomainError::InvalidSampleRate(u32)` ([src/domain/error.rs:6](../../../../src/domain/error.rs)) is reused as-is. `SampleRate` wraps `f32` ([src/domain/types.rs:3](../../../../src/domain/types.rs)) but the variant carries a `u32` for display convenience — `measure_latency` casts via `sr.value() as u32` when constructing the error. No signature change to the existing variant; only two new sibling variants.

### Errors

| Error                                  | Variant of    | Where translated to user                           | Status |
| -------------------------------------- | ------------- | -------------------------------------------------- | ------ |
| `DomainError::LatencyNoPeak`           | Domain        | (mvp-02) GUI sentinel `latency: no signal`        | 🟢     |
| `DomainError::LoopbackTooShort`        | Domain        | (mvp-02) GUI error path; also a unit test         | 🟢     |
| `DomainError::InvalidSampleRate(0)`    | Domain        | (reused) caller responsibility                     | ⚪     |

Per rule D2 these stay as a domain error type — the audio adapter (mvp-02) translates display strings at the boundary.

### Persistence

No persistence change.

---

## 4. Architecture

### 4.1 Domain — pure core

> Rule A1 reminder: zero imports from audio I/O, GUI, plugin host, or filesystem crates in this layer. The primitive uses only `core::f32`, the existing `SampleRate` NewType, and `DomainError`.

```diff
domain::latency::measure_latency
├── 🟢 🧩 measure_latency(&[f32], &[f32], SampleRate) -> Result<LatencyMs, DomainError>   src/domain/latency.rs
+         O(N·M) sliding-window cross-correlation; argmax magnitude; threshold test for silence.
+         Slice-only kernel — no Vec, no Box, no allocator call.
│   ├── 🟢 🎯 LatencyMs(f32)                                                                src/domain/latency.rs
+              NewType per rule C5; one-decimal rounding in `new`.
│   ├── ⚪ 🎯 SampleRate                                                                    src/domain/types.rs
+              Reused as-is.
│   └── 🟡 🎯 DomainError (LatencyNoPeak, LoopbackTooShort)                                 src/domain/error.rs
+              Two new variants; existing InvalidSampleRate reused.
└── 🟡 📂 mod.rs (`pub mod latency;`)                                                        src/domain/mod.rs
+      Add the module declaration.
```

#### Domain rules at play

- **Tell, don't ask** (C1): `measure_latency` returns the result; callers do not pull internal correlation arrays out.
- **Parse, don't validate** (E2): preconditions (length, SR ≠ 0, peak above threshold) are checked once at function entry; downstream callers receive a `LatencyMs` carrying the guarantee.
- **NewType for primitives** (C5): `LatencyMs` private field + `value()` accessor mirrors `Decibels` and `SampleRate` style.
- **No `unwrap`** (D1, D3): every fallible step uses `?` or returns `Err` explicitly.

#### Algorithm

```text
measure_latency(reference: &[f32], loopback: &[f32], sr: SampleRate) -> Result<LatencyMs, DomainError>:
  if sr.value() <= 0.0 → Err(InvalidSampleRate(sr.value() as u32))
  if loopback.len() < reference.len() → Err(LoopbackTooShort)
  let max_lag = loopback.len() - reference.len()
  let mut best_lag = 0; let mut best_corr = 0.0;
  for lag in 0..=max_lag:
    let corr = Σ_{i ∈ 0..reference.len()} reference[i] * loopback[lag + i]
    if corr.abs() > best_corr.abs() { best_corr = corr; best_lag = lag; }
  let loopback_peak = loopback.iter().fold(0.0_f32, |a, &x| a.max(x.abs()))
  let threshold = 0.1 * loopback_peak * reference.iter().map(|x| x.abs()).sum::<f32>()
  if best_corr.abs() < threshold → Err(LatencyNoPeak)
  let ms_raw = (best_lag as f32 / sr.value()) * 1000.0
  Ok(LatencyMs::new(ms_raw))   // rounds to 1 decimal in `new`
```

Complexity: O(N·M) where N = `reference.len()`, M = `loopback.len() − reference.len()`. For a 1024-sample reference and a 4096-sample loopback at 48 kHz that is ~12 M multiply-adds — well under one ms on the dev machine, and called once per measurement.

### 4.2 Audio adapter — realtime shell

No audio adapter change. (mvp-02 wires capture and emission.)

### 4.3 Control surface — GUI / MIDI

No control-surface change.

### 4.4 Persistence

No persistence change.

### 4.5 Composition root

🟡 [src/domain/mod.rs](../../../../src/domain/mod.rs) declares `pub mod latency;`. No DI wiring — `measure_latency` is a free function. Callers (mvp-02) `use tonism::domain::latency::{measure_latency, LatencyMs};`.

### 4.6 Key technical decisions

- **Free function over a struct** — there is no per-call state to amortize and the standards favour functional purity (rule C3). A `LatencyMeter` struct would only buy ergonomics that mvp-02 does not need.
- **Threshold = 10 % of `peak(loopback) × Σ|reference|`** — empirical: at unity loopback this rejects all-zero or noise-only inputs while accepting any clean impulse echo. Tunable in a follow-up if the manual run on real hardware exposes sensitivity issues.
- **One-decimal rounding inside `LatencyMs::new`** — keeps the AC's rounding contract local to the type instead of every call site. Format-string rounding in the GUI is also fine but redundant.
- **Reuse `DomainError` over a local `LatencyError`** — only two new variants and they belong to the same domain. A separate enum would add a `From` impl boundary for no clear gain.

---

## 5. Tests

### 5.1 e2e (audio path)

No e2e impact.

### 5.2 Integration

No new integration test in this story. mvp-02 owns the end-to-end "impulse → audio adapter → capture → measure" integration test; mvp-01 stops at the pure-function boundary.

### 5.3 Unit (pure domain) — co-located in `src/domain/latency.rs` per rule G5

- 🟢 `measure_latency_recovers_synthetic_delay_at_boundary_rates` — table-driven over `(delay, sr) ∈ {0, 32, 256, 2048} × {44_100, 48_000, 88_200, 96_000}`. Builds `reference = kronecker_impulse(1024)`, builds `loopback` as zeros prefix of `delay` samples + `reference` + zeros suffix to total length 8192. Asserts `result.value() == round_to_1dp((delay as f32 / sr) * 1000.0)`. (16 cases.)
- 🟢 `silent_loopback_returns_no_peak` — `loopback = vec![0.0; 8192]` → `Err(DomainError::LatencyNoPeak)`.
- 🟢 `loopback_shorter_than_reference_returns_too_short` → `Err(DomainError::LoopbackTooShort)`.
- 🟢 `sample_rate_zero_returns_invalid_sample_rate` → `Err(DomainError::InvalidSampleRate(0))`.
- 🟢 `latency_ms_new_rounds_to_one_decimal` — `LatencyMs::new(7.36).value() == 7.4`.

The `kronecker_impulse(n)` fixture lives in `tests/common/fixtures.rs` ([fixtures.rs:15](../../../../tests/common/fixtures.rs)) and is `pub`; co-located unit tests in `src/domain/latency.rs` cannot import from `tests/common/`. Two options: (a) duplicate the 7-line fixture inside the unit test module, or (b) copy `kronecker_impulse` into a `pub fn` in `src/domain/latency.rs::test_helpers` and have `tests/common/fixtures.rs` re-export it. Option (a) is simpler and 7 lines duplicated is below any reasonable abstraction threshold. Use (a).

### 5.4 AC coverage table

| AC / Checklist item                                                  | Test                                                       |
| -------------------------------------------------------------------- | ---------------------------------------------------------- |
| Recovers N at SR ∈ {44.1, 48, 88.2, 96} kHz                          | unit `measure_latency_recovers_synthetic_delay_at_boundary_rates` |
| One-decimal-rounded ms                                               | unit `latency_ms_new_rounds_to_one_decimal`                |
| Silent loopback → Err                                                | unit `silent_loopback_returns_no_peak`                     |
| Loopback shorter → Err                                               | unit `loopback_shorter_than_reference_returns_too_short`   |
| `SampleRate(0)` → Err                                                | unit `sample_rate_zero_returns_invalid_sample_rate`        |
| Alloc-free in kernel                                                 | reading review of the `for lag in 0..=max_lag` body        |
| `cargo clippy --all-targets -- -D warnings`                          | CI + lefthook                                              |

---

## 6. Dependencies and execution order

- **Prerequisite stories**: none. Can start immediately.
- **Stories unblocked**: [mvp-02](mvp-02-latency-readout-in-standalone.md) consumes `domain::latency::measure_latency`.
- **Commands to run** (local + CI):
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test latency`
  - `cargo test --features debug-assert-no-alloc latency`

---

## 7. Risks and open questions

- 🟢 Pure-domain story; no realtime constraints, no GUI work, no host integration. Lowest-risk story in the spec.
- 🟡 Cross-correlation peak detection at extreme corners (delay = 0 with non-zero noise floor in a future real-hardware run) may need a tighter threshold. Boundary unit tests cover synthetic cases; the manual mvp-02 run is the real-world check.
- 🟡 Reusing `DomainError::InvalidSampleRate(u32)` with `sr.value() as u32` is a small lossy cast (negative or non-finite SRs round to 0). Documented in the function precondition; not a problem for the AC.

---

## 8. References

- Similar implementations to follow: existing NewType + `From` style in [`src/domain/types.rs`](../../../../src/domain/types.rs); error style in [`src/domain/error.rs`](../../../../src/domain/error.rs); co-located unit tests style in [`src/domain/blocks/gain.rs`](../../../../src/domain/blocks/gain.rs).
- Directly applicable standards: [architecture.md](../../../standards/architecture.md) (A1, A4), [domain.md](../../../standards/domain.md) (C5, D1, D2, D3, E2), [testing.md](../../../standards/testing.md) (G5, G7).
- Related ADRs: [ADR-001](../../../adr/001-language-choice.md) (Rust + thiserror trajectory).
- Product layers and success bars: [product-architecture.md](../../product-architecture.md).
- Source spec section: [acceptance criteria AC1](../spec.md#acceptance-criteria); [dependencies — latency measurement](../dependencies.md#patterns).
