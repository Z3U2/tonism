# Implementation plan: cpal-direct-standalone-F01 — Bypass toggle and 440 Hz test-signal

**Story**: [cpal-direct-standalone-F01-bypass-and-test-signal.md](cpal-direct-standalone-F01-bypass-and-test-signal.md)
**Spec**: [spec.md](../spec.md)
**Layers**: Capture · Render · ~~Signal chain~~ · ~~Tone state~~ · ~~Control surface~~ · ~~Persistence~~ · Tests
**Complexity**: 🟢 Low

---

## 1. Summary

Wire the two existing `BoolParam`s (`bypass`, `test_signal`) — currently destructured as `_` in `setup_audio()` — into the cpal-direct input and output callbacks. Bypass skips all gain/processing stages; test signal replaces mic input with a 440 Hz sine via a phase accumulator. The main challenge is keeping the callbacks A2-clean while reading two additional atomics per buffer.

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case | Layer | Main component | Status |
| --- | --- | --- | --- |
| Bypass OFF, test signal OFF: existing path unchanged | Render | `src/cpal_direct.rs` output closure | ⚪ |
| Bypass ON: no gain applied, signal passthrough | Capture+Render | `src/cpal_direct.rs` input + output closures | 🟡 |
| Test signal ON, bypass OFF: 440 Hz through full chain | Capture | `src/cpal_direct.rs` input closure (sine gen) | 🟡 |
| Test signal ON, bypass ON: 440 Hz at unity | Capture+Render | `src/cpal_direct.rs` both closures | 🟡 |
| Bypass during latency capture → cancel() | Render | `src/cpal_direct.rs` output closure | 🟡 |
| `cargo run` — toggle bypass | Composition root | `src/cpal_direct.rs` run_gui path | ⚪ |
| `cargo test` — no regression | Tests | existing test suite | ⚪ |

### Out of scope (declared)

- The `cancel()` call site is placed but is a no-op until F02 adds `LatencyMeter` to the callback. The full cancel-during-capture scenario is testable only after F02 merges.

---

## 3. Domain & data model

No domain change. No new types.

`BoolParam` (in `src/params.rs`) is already constructed for both `bypass` and `test_signal` — the atomics exist; only the callback reads are missing.

No persistence change.

---

## 4. Architecture

### 4.1 Domain — pure core

No domain changes. The following are reused as-is:

- ⚪ `SampleRate` — `src/domain/types.rs`
- ⚪ `Decibels`, `GainLinear` — `src/domain/types.rs`
- ⚪ `Gain` block — `src/domain/blocks/gain.rs`
- ⚪ `Process` trait — `src/domain/process.rs`

---

### 4.2 Audio adapter — realtime shell

#### Components

```diff
src/cpal_direct.rs — setup_audio()
├── 🟡 input_data_fn closure (lines 159–179)
│   + Read test_signal BoolParam (atomic load, Relaxed)
│   + When ON: generate sine samples from phase accumulator instead of mic input
│   + Read bypass BoolParam (atomic load, Relaxed)
│   + When bypass ON: skip input_gain multiply, push raw (or sine) samples to ring
│   └── 🟢 Phase accumulator (f32, moved into closure)
│        Advances by 2π × 440 / sample_rate per frame. Wraps with % TAU.
├── 🟡 output_data_fn closure (lines 183–211)
│   + Read bypass BoolParam (atomic load, Relaxed)
│   + When bypass ON: skip gain_block.process(data) and output_gain multiply
│   + Call latency_meter.cancel() when bypass ON (no-op until F02)
│   └── ⚪ gain_block, output_gain_audio (existing, skipped under bypass)
└── 🟡 TonismParamsAudio destructure (line 147–152)
    - Change `bypass: _` → `bypass` (move into both closures via clone)
    - Change `test_signal: _` → `test_signal` (move into input closure)
```

#### Signal ordering (matches nih-plug `plugin.rs`)

**Input callback:**
1. Read `test_signal`. If ON: generate sine per frame, replace `data[frame_start + ch]`.
2. Read `bypass`. If OFF: apply `input_gain` multiply per frame. If ON: skip.
3. Push to ring.

**Output callback:**
1. Drain ring → `data`.
2. Read `bypass`. If ON: call `latency_meter.cancel()` (F02 placeholder), return early.
3. `gain_block.process(data)`.
4. Apply `output_gain` per frame.

#### Realtime constraints checklist

- [x] No `Vec::push`, `Box::new`, or other heap allocation inside the per-buffer path.
- [x] No `Mutex`, `RwLock`, or other blocking primitive.
- [x] No filesystem, network, or `println!` calls.
- [x] `BoolParam::value()` is a single `AtomicBool::load(Relaxed)` — no alloc, no lock.
- [x] Phase accumulator is a stack `f32` moved into the closure — no heap.
- [x] `f32::sin()` is a CPU instruction, no syscall.

---

### 4.3 Control surface — GUI / MIDI

No control-surface change. The bypass and test-signal checkboxes already exist in `src/gui/standalone.rs` (lines 54–62) and write to the same `BoolParam` atomics. This story adds the *read* side in the audio callbacks.

---

### 4.4 Persistence

No persistence change.

---

### 4.5 Composition root

- 🟡 `src/cpal_direct.rs` `setup_audio()` — stop discarding `bypass` and `test_signal` from `TonismParamsAudio`. Clone `bypass` for both closures; move `test_signal` into the input closure.
- 🟢 Compute `phase_inc` (`TAU * 440.0 / sr.value()`) once in `setup_audio()`, move into input closure.
- 🟢 Allocate `phase: f32 = 0.0` moved into input closure.

---

### 4.6 Key technical decisions

- **Sine in the input callback, not the output callback**: matches nih-plug Stage 1 ordering. The sine flows through the full chain (input_gain → ring → gain_block → output_gain) when not bypassed, so latency measurement in F02 will see it.
- **Bypass reads are per-buffer, not per-frame**: one `bypass.value()` load at the top of each closure. A bypass toggle mid-buffer takes effect at the next buffer boundary — acceptable at < 5 ms buffer sizes.
- **`latency_meter.cancel()` placeholder**: F01 inserts the call site in the bypass branch of the output callback. Until F02 adds the meter, this is a comment marking where the call goes (or a no-op function if the LatencyMeter is added to the closure in F02's PR).

---

## 5. Tests

### 5.1 e2e (audio path)

No new e2e test. The 5-minute manual session covers this.

### 5.2 Integration

No new integration test file. The bypass and test-signal logic is too tightly coupled to the cpal callback closures (which own the ring + domain block) to test via `BufferBackend`. The existing `latency_meter_round_trip.rs` and `smoke.rs` continue passing (no regression).

### 5.3 Unit (pure domain)

No new unit tests — no domain change.

### 5.4 Inline validation

The bypass and test-signal paths are validated manually per the checklist. A dedicated integration test for bypass is deferred to F02, where the full signal path (with latency meter) can be driven synthetically.

### 5.5 AC coverage table

| AC / Checklist item | Validation |
| --- | --- |
| Bypass OFF, test signal OFF: unchanged | `cargo test` (existing suite) + manual |
| Bypass ON: no gain | Manual: toggle checkbox, listen |
| Bypass OFF → no click | Manual: toggle back, listen |
| Test signal ON: 440 Hz tone | Manual: toggle, listen without mic |
| Both ON: tone at unity | Manual: toggle both, listen |
| `cargo test` no regression | `cargo test` (CI gate) |

---

## 6. Dependencies and execution order

- **Prerequisite stories**: None — F01 is the first in the Phase F chain.
- **Stories unblocked**: F02 (LatencyMeter in callback) depends on F01's bypass path being wired so the cancel-during-capture scenario works.
- **Commands to run**:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo run` — manual 5-minute session with active bypass/test-signal toggling

---

## 7. Risks and open questions

- 🟢 **Low risk** — bypass and test-signal are single-atomic reads. The entire change is < 30 lines of callback logic plus the phase accumulator.
- ��� **Phase accumulator precision** — `f32` phase wrapping with `% TAU` can accumulate rounding error over long sessions. Acceptable for a test signal; if it drifts audibly after hours, the fix is `phase -= TAU` instead of `% TAU` (same as nih-plug plugin.rs line 214). Use the same pattern.
- 🟡 **`cancel()` placeholder** — the call site exists but is dead code until F02 lands. `#[allow(unused)]` or a comment is needed to avoid a clippy warning. Alternatively, defer the cancel call site to F02's PR entirely and note the dependency.

---

## 8. References

- Signal ordering reference: [`src/audio/plugin.rs:191–246`](../../../../src/audio/plugin.rs) — nih-plug `process()` with bypass, test-signal, gain stages.
- Parameter system: [`src/params.rs:170–199`](../../../../src/params.rs) — `BoolParam` with `value()` (atomic read).
- Architecture rule A2: [architecture.md](../../../standards/architecture.md) — no alloc/lock/syscall on audio thread.
- ADR-005: [005-standalone-audio-cpal-direct.md](../../../adr/005-standalone-audio-cpal-direct.md) — the pivot decision.
