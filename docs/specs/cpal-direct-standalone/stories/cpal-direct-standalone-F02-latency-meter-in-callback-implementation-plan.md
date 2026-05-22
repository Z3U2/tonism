# Implementation plan: cpal-direct-standalone-F02 — LatencyMeter wired into cpal output callback

**Story**: [cpal-direct-standalone-F02-latency-meter-in-callback.md](cpal-direct-standalone-F02-latency-meter-in-callback.md)
**Spec**: [spec.md](../spec.md)
**Layers**: Capture · Render · ~~Signal chain~~ · ~~Tone state~~ · ~~Control surface~~ · ~~Persistence~~ · Tests
**Complexity**: 🟡 Medium

---

## 1. Summary

Plumb the existing `LatencyMeter` into the cpal-direct output callback so it captures loopback on channel 0 and emits impulses, enabling round-trip latency measurement. The central challenge is the **deinterleave question**: `LatencyMeter::process(&mut [f32])` expects a contiguous mono buffer, but cpal delivers interleaved multi-channel data. Solved with a pre-allocated scratch buffer and extract/write-back on channel 0 only. The `LatencyHandle` is surfaced in `AudioSession` for F03 to wire to the GUI.

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case | Layer | Main component | Status |
| --- | --- | --- | --- |
| LatencyMeter receives ch0-only samples via scratch buffer | Render | `src/cpal_direct.rs` output closure | 🟡 |
| Armed → Idle → Capturing → Done after CAPTURE_LEN ch0 samples | Render | `src/audio/latency.rs` (reused) | ⚪ |
| Capture buffer valid for `measure_latency()` on loopback | Domain | `src/domain/latency.rs` (reused) | ⚪ |
| Impulse emission on ch0 only; other channels unaffected | Render | `src/cpal_direct.rs` write-back logic | 🟡 |
| Bypass ON → `cancel()` called | Render | `src/cpal_direct.rs` bypass branch (from F01) | 🟡 |
| `LatencyHandle` in `AudioSession` | Composition root | `src/cpal_direct.rs` struct + setup_audio | 🟡 |
| Integration test: synthetic drive → Done + valid capture | Tests | `tests/latency_meter_round_trip.rs` | 🟡 |
| `cargo run --bin feedback` — no regression | Render | `src/bin/feedback.rs` (unchanged) | ⚪ |
| `cargo run` — clean audio, meter idle | Render | `src/cpal_direct.rs` | 🟡 |

### Out of scope (declared)

- GUI button and readout — delivered in F03.
- The `--measure` dev flag mentioned in the checklist is optional; the integration test is the primary verification of the meter's audio-side behaviour.

---

## 3. Domain & data model

No domain change. No new types.

Reused as-is:
- ⚪ `LatencyMeter` — `src/audio/latency.rs` (implements `Process`)
- ⚪ `LatencyHandle` — `src/audio/latency.rs` (GUI-side facet, three Arc clones)
- ⚪ `measure_latency` — `src/domain/latency.rs`
- ⚪ `CaptureState` — `src/audio/latency.rs`
- ⚪ `CAPTURE_LEN`, `N_IMPULSES`, `IMPULSE_INTERVAL` — `src/audio/latency.rs`

No persistence change.

---

## 4. Architecture

### 4.1 Domain — pure core

No domain changes. All domain components reused as-is (see section 3).

---

### 4.2 Audio adapter — realtime shell

#### Components

```diff
src/cpal_direct.rs — setup_audio()
├── 🟢 ch0_scratch: Vec<f32>
│   Pre-allocated in setup_audio() with capacity MAX_BLOCK_SIZE (65536).
│   Moved into the output closure. Reused each callback via &mut ch0_scratch[..n_frames].
│   No per-callback allocation.
│
├── 🟢 LatencyMeter (instantiated via Default::default())
│   Constructed in setup_audio(). prepare() + reset() called before stream start.
│   Moved into the output closure.
│
├── 🟡 output_data_fn closure (lines 183–211)
│   After ring drain, before gain_block.process(data):
│   1. Extract ch0: for i in 0..n_frames { ch0_scratch[i] = data[i * channels]; }
│   2. latency_meter.process(&mut ch0_scratch[..n_frames]);
│   3. Write-back ch0: for i in 0..n_frames { data[i * channels] = ch0_scratch[i]; }
│   4. (existing) gain_block.process(data)
│   5. (existing) output_gain per frame
│   Under bypass (from F01): call latency_meter.cancel(), skip steps 1–5.
│
├── 🟡 AudioSession struct (line 90–95)
│   + latency_handle: LatencyHandle field
│
└── 🟡 setup_audio() return
    + Create LatencyMeter, call .handle() for the GUI side
    + Prepare the meter: latency_meter.prepare(sr, MAX_BLOCK_SIZE)
    + Return latency_handle in AudioSession
```

#### Signal ordering (output callback)

```
drain ring → data
    │
    ├─ if bypass: latency_meter.cancel(), return (passthrough)
    │
    ├─ extract ch0 → ch0_scratch[..n_frames]
    ├─ latency_meter.process(&mut ch0_scratch[..n_frames])
    ├─ write-back ch0_scratch → data (ch0 positions only)
    │
    ├─ gain_block.process(data)          ← all channels
    └─ output_gain per frame             ← all channels
```

This matches nih-plug ordering: input_gain → latency_meter(ch0) → gain_block → output_gain.

#### Deinterleave detail

```rust
let n_frames = data.len() / channels;
// Extract channel 0
for i in 0..n_frames {
    ch0_scratch[i] = data[i * channels];
}
// Process (capture + impulse emission on ch0)
latency_meter.process(&mut ch0_scratch[..n_frames]);
// Write back channel 0 only
for i in 0..n_frames {
    data[i * channels] = ch0_scratch[i];
}
```

- Cost: 2 × n_frames copies per callback. At 48 kHz / 256 frames = ~2 μs. Negligible.
- Other channels (`data[i * channels + 1..channels]`) are untouched — impulses only appear on ch0.

#### Scratch buffer sizing

`MAX_BLOCK_SIZE` = 65536. This is the upper bound on `data.len()` (interleaved). `n_frames = data.len() / channels`. Worst case (mono): n_frames = 65536. The scratch Vec is allocated once at 65536 × 4 bytes = 256 KB. Acceptable for a desktop app.

#### Realtime constraints checklist

- [x] No heap allocation inside the per-buffer path. `ch0_scratch` is pre-allocated; slice indexing only.
- [x] No `Mutex`, `RwLock`, or other blocking primitive.
- [x] No filesystem, network, or `println!` calls.
- [x] `LatencyMeter::process()` uses only atomic stores (Relaxed/Release) and a `write_idx` counter — A2-safe by construction (verified in existing unit tests).
- [x] `cancel()` is a single `compare_exchange` on an `AtomicU8` — A2-safe.
- [x] The scratch buffer is a `Vec<f32>` moved into the closure; `&mut ch0_scratch[..n]` is a slice borrow, not an allocation.

---

### 4.3 Control surface — GUI / MIDI

No control-surface change in this story. `LatencyHandle` is exposed in `AudioSession` so F03 can pass it to `TonismApp`.

---

### 4.4 Persistence

No persistence change.

---

### 4.5 Composition root

- 🟡 `AudioSession` struct — add `latency_handle: LatencyHandle` field.
- 🟡 `setup_audio()` — construct `LatencyMeter::default()`, call `prepare(sr, MAX_BLOCK_SIZE)` and `reset()`, extract `.handle()`, move meter into output closure, return handle in session.
- 🟡 `run_gui()` — destructure `latency_handle` from `AudioSession` (passed to GUI in F03; unused in this story but present).
- ⚪ `run()` (headless) — no change; the handle is constructed but unused.

---

### 4.6 Key technical decisions

- **Pre-allocated scratch Vec over in-place strided processing**: `LatencyMeter` implements `Process` which expects `&mut [f32]` — a contiguous slice. Changing the trait to support strided access would modify the domain-adjacent `Process` trait, violating the "domain untouched" constraint. The 256 KB pre-allocation is trivial for a desktop app and the copy cost (~2 μs) is well within the audio budget.
- **Meter placement: after ring drain, before gain_block**: matches the nih-plug signal path. The meter sees the loopback return (signal that went out → came back in → flowed through ring). The emitted impulse is then processed by gain_block and output_gain, which is fine — it exits at the output device.
- **`cancel()` in bypass branch**: when bypass is ON, the output callback returns early (from F01). Before returning, it calls `latency_meter.cancel()` so an in-progress capture transitions to `Cancelled` and the GUI can display the sentinel.

---

## 5. Tests

### 5.1 e2e (audio path)

No new e2e test. The 5-minute manual session with the meter idle covers "no regression."

### 5.2 Integration (load-bearing)

- 🟡 **Extend `tests/latency_meter_round_trip.rs`** — the existing tests already validate meter state transitions and `measure_latency()` recovery via `BufferBackend`. They drive `LatencyMeter` directly through `Process`. This is the same path the cpal-direct output callback uses (minus the deinterleave, which is tested in a new test below).

- 🟢 **New test: `deinterleave_round_trip`** — validate that the extract-process-writeback pattern on an interleaved buffer produces correct results:
  - Construct a synthetic interleaved buffer (2 channels, N frames) with a known delay on channel 0.
  - Simulate the deinterleave + `LatencyMeter::process` + write-back.
  - Assert: channel 0 has impulses at expected positions; channel 1 is unchanged.
  - Assert: capture buffer has the expected delay pattern.
  - Location: `tests/latency_meter_round_trip.rs` (extends the existing file).

### 5.3 Unit

No new unit tests in domain. The existing 12 tests in `src/audio/latency.rs` and 7 in `src/domain/latency.rs` cover the meter's internal state machine.

### 5.4 AC coverage table

| AC / Checklist item | Test |
| --- | --- |
| Meter receives ch0-only samples | integration `deinterleave_round_trip` |
| Armed → Idle → Capturing → Done | integration (existing `round_trip_with_synthetic_delay_via_buffer_backend`) |
| Capture valid for `measure_latency()` | integration (existing + new deinterleave test) |
| Impulses on ch0 only, other channels unaffected | integration `deinterleave_round_trip` — assert ch1 unchanged |
| Bypass → cancel() | manual (toggle bypass during a measurement in F03) |
| `cargo run --bin feedback` no regression | `cargo test` + manual |
| `cargo run` clean audio, meter idle | manual |

---

## 6. Dependencies and execution order

- **Prerequisite stories**: F01 must land first — F02's bypass branch calls `latency_meter.cancel()` which depends on the bypass-read logic from F01.
- **Stories unblocked**: F03 (latency GUI) can start once `AudioSession.latency_handle` is available.
- **Commands to run**:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test` (includes the new integration test)
  - `cargo run` — manual session, confirm no xruns with meter idle
  - `cargo run --features debug-assert-no-alloc` — confirm no alloc panic with the new scratch-buffer path

---

## 7. Risks and open questions

- 🟢 **Deinterleave pattern is well-understood** — extract/write-back on a pre-allocated scratch buffer is the standard solution for channel-specific processing on interleaved data. No invention needed.
- 🟢 **LatencyMeter is A2-safe by construction** — existing unit test `meter_alloc_free_steady_state` confirms no allocation; the atomic operations are all Relaxed/Release.
- 🟡 **Mono input device** — if the input device is mono (1 channel) and the output is stereo (2 channels), `n_frames` in the output callback = `data.len() / 2`. Channel 0 extraction still works correctly. The meter emits impulses on channel 0 of the output; the loopback captures whatever arrives on channel 0 of the input — which is the full mono input. This is the MVP's primary shape (mono in, stereo out).
- 🟡 **Scratch buffer under `debug-assert-no-alloc`** — the `Vec` is pre-allocated before the stream starts. The `&mut ch0_scratch[..n_frames]` slice borrow inside the callback is not an allocation. Verify with `cargo run --features debug-assert-no-alloc` that no panic fires.

---

## 8. References

- Deinterleave pattern: the extract/write-back loop is the same shape as the per-frame gain loop already in `src/cpal_direct.rs:196–206`.
- LatencyMeter: [`src/audio/latency.rs:62–165`](../../../../src/audio/latency.rs) — full Process impl.
- Integration test to extend: [`tests/latency_meter_round_trip.rs`](../../../../tests/latency_meter_round_trip.rs).
- Architecture rule A2: [architecture.md](../../../standards/architecture.md) — no alloc/lock/syscall on audio thread.
- Testing standard G4: [testing.md](../../../standards/testing.md) — real dependencies over deep mocks.
