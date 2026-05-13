# Implementation plan: mvp-02 — Latency readout in the standalone

**Story**: [mvp-02-latency-readout-in-standalone.md](mvp-02-latency-readout-in-standalone.md)
**Spec**: [spec.md](../spec.md)
**Layers**: Capture · Signal chain · Render · ~~Tone state~~ · Control surface · ~~Persistence~~ · Tests
**Complexity**: 🔴 High

---

## ✅ Decisions taken

The two TR decisions previously flagged in [stories.md](../stories.md) are resolved.

### D1 — Architecture: Option O (`LatencyMeter` as a `Process` block)

The capture state machine lives in a new `LatencyMeter` block that implements the existing [`Process`](../../../../src/domain/process.rs) trait, not in `TonismPlugin` glue. The block sits at the start of the chain (before `input_gain`); on each `process(buffer)` call it captures the inbound buffer (the loopback signal) and, while a measurement is in flight, overwrites the buffer with a Kronecker impulse on the first frame of the capture window. The impulse then travels through the rest of the chain to the audio output.

**Why O over A** (see [discussion above](../stories.md#notes-for-the-technical-refinement)): the block pattern is the v0.2 chain shape anyway; establishing it now with `Gain` and `LatencyMeter` as the first two blocks costs nothing extra and pays back when v0.2 adds more blocks. Crucially, **`LatencyMeter` becomes testable through `BufferBackend` exactly like `Gain`** — closing the testability gap flagged in [mvp-04 §4.6](mvp-04-idle-5min-stability-test-implementation-plan.md) and [mvp-05 §4.6](mvp-05-stress-30min-test-implementation-plan.md).

### D2 — Synchronisation: Option A (`AtomicU32` per sample + `AtomicU8` state)

Inside the block, the capture buffer is `Box<[AtomicU32; CAPTURE_LEN]>` (each slot stores one `f32` via `to_bits()` / `from_bits()`); the state is `Arc<AtomicU8>` with `Idle = 0`, `Capturing = 1`, `Done = 2`. The trigger from GUI is `Arc<AtomicBool>` (no nih-plug `BoolParam` needed — keeps the GUI-shared interface fully self-contained and mirrors the [`XrunCounter`](../../../../src/audio/xrun.rs) pattern).

**Why A over K (`UnsafeCell` + state-as-synchroniser)**: the per-sample `AtomicU32::store(Release)` cost is negligible compared to the once-per-measurement compute; `UnsafeCell` would shave that cost at the price of a soundness comment that has to be re-derived by future reviewers. Pay the small inner-loop cost; bank the simplicity.

**Why A over B (`triple_buffer`) / C (`rtrb` data ring)**: for a one-shot 4096-sample capture, the array-shaped read on the GUI side is simpler than draining a queue and does not pull in any new dependency.

### D3 — Sentinel display states (resolved)

`LatencyDisplay` enum: `Pending` → "latency: -- ms"; `Measuring` → "latency: measuring..."; `Measured(LatencyMs)` → "latency: 7.3 ms"; `NoSignal` → "latency: no signal"; `Cancelled` → "latency: -- ms".

### D4 — Capture window length (resolved)

`CAPTURE_LEN = 4096` — covers > 90 ms at the lowest supported SR (44.1 kHz), comfortable headroom over the < 10 ms dev-machine target. Fixed across SRs to keep the capture buffer one allocation.

---

## 1. Summary

Wire a "Measure latency" button in the standalone editor that arms a new `LatencyMeter` `Process` block. When armed, the block emits one Kronecker impulse on the first frame of its next `process(buffer)` call (which then propagates through `input_gain → Gain → output_gain` to the audio output), captures the inbound loopback for 4096 samples into a lock-free shared buffer, and signals the GUI when complete. The GUI polls the block's atomic state at 60 Hz, runs [mvp-01](mvp-01-latency-measurement-primitive.md)'s `measure_latency` over the captured samples, and replaces the existing `latency: -- ms` placeholder ([editor.rs:88](../../../../src/gui/editor.rs)) with the measured ms (or a `no signal` sentinel). The technical edge is keeping the block A2-safe (no alloc/lock/syscall on the per-buffer path) while remaining testable through `BufferBackend` exactly like the existing `Gain` block.

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case                                                                    | Layer           | Main component                                                       | Status |
| ----------------------------------------------------------------------------------- | --------------- | -------------------------------------------------------------------- | ------ |
| Click "Measure latency" → ms readout shows within 2 s, value < 10 ms with loopback | Control surface | `Button` widget + `LatencyMeter::handle()` poll                      | 🟢     |
| Click → audio thread emits Kronecker impulse on output, captures N samples         | Signal chain    | `LatencyMeter::process()` impulse-emit + atomic capture              | 🟢     |
| Readout updates after each click; persists between clicks                           | Control surface | `SyncSignal<LatencyDisplay>` + 60 Hz poll                            | 🟢     |
| Toggle bypass while idle: no effect on readout                                      | Signal chain    | `LatencyMeter` is independent of bypass; existing bypass branch unchanged | ⚪     |
| Toggle bypass during measurement: clean cancel                                      | Signal chain    | `LatencyMeter::cancel()` called by `TonismPlugin` when bypass flips on | 🟢     |
| Silent input → `latency: no signal` sentinel                                        | Control surface | `LatencyDisplay::NoSignal` Memo branch                               | 🟢     |
| Audio device fails to start → label stays in placeholder, no crash                  | (existing)      | nih-plug existing error path; capture state untouched                | ⚪     |
| Second click while measuring → ignored, no overlap                                  | Signal chain    | `LatencyMeter::arm` checks `state != Idle` and ignores              | 🟢     |
| Manual: status area / log shows buffer size, sample rate, audio backend, OS        | Render          | `tracing::info!` line on `Plugin::initialize`                        | 🟡     |
| `--features debug-assert-no-alloc` + measure → no allocation panic                  | Signal chain    | All capture state pre-allocated in `LatencyMeter::default()`         | 🟢     |

### Out of scope (declared)

- Repurposing or removing the existing 440 Hz `test_signal` toggle ([params.rs:19–20](../../../../src/audio/params.rs)) — kept as-is; useful for AC4 audible verification, independent of latency measurement.
- Auto-running latency on stream start — AC1 protocol is explicitly user-triggered.
- Per-channel latency display — capture only channel 0; multi-channel divergence is a v0.2 concern.
- Adding `LatencyMeter` to mvp-04 / mvp-05's stress harnesses — those stories stay scoped on `Gain`. A follow-up story can extend the harnesses once `LatencyMeter` lands.

---

## 3. Domain & data model

### New / modified types

| Change                                  | Description                                                                | Status |
| --------------------------------------- | -------------------------------------------------------------------------- | ------ |
| ⚪ `LatencyMs`, `measure_latency`       | Reused as-is from [mvp-01](mvp-01-latency-measurement-primitive.md)        | ⚪     |
| 🟢 Enum `CaptureState { Idle, Capturing, Done }` (`#[repr(u8)]`) | Discriminated union (rule E4) backing the `AtomicU8` state                 | 🟢     |
| 🟢 Struct `LatencyMeter`                | Audio-shell `Process`-implementing block; owns capture state + atomics     | 🟢     |
| 🟢 Struct `LatencyHandle`               | GUI-side facet of `LatencyMeter`: cloned `Arc`s + safe accessors           | 🟢     |
| 🟢 Enum `LatencyDisplay`                | GUI presentation: `Pending`, `Measuring`, `Measured(LatencyMs)`, `NoSignal`, `Cancelled` | 🟢     |
| ⚪ `DomainError::LatencyNoPeak`         | Reused as-is from mvp-01                                                   | ⚪     |

`LatencyMeter` and `LatencyHandle` live under `src/audio/latency.rs` (audio shell, mirroring [`src/audio/xrun.rs`](../../../../src/audio/xrun.rs)) — **not** under `src/domain/blocks/` because they hold `Arc<Atomic*>` shared state with the GUI. Per rule A1 the domain stays free of concurrency-shared state; per rule A6 the audio adapter owns the GUI ↔ audio messaging primitive.

`LatencyDisplay` lives in [`src/gui/editor.rs`](../../../../src/gui/editor.rs) as a private enum used by the `Memo` formatter — it is presentation, not domain.

### Errors

| Error                                  | Variant of    | Where translated to user                            | Status |
| -------------------------------------- | ------------- | --------------------------------------------------- | ------ |
| `DomainError::LatencyNoPeak`           | Domain        | GUI: `LatencyDisplay::NoSignal` → "latency: no signal" | ⚪     |
| `DomainError::LoopbackTooShort`        | Domain        | Should not occur (capture length is fixed) — log only | ⚪     |

### Persistence

No persistence change.

---

## 4. Architecture

### 4.1 Domain — pure core

> Rule A1 reminder: zero imports from audio I/O, GUI, plugin host, or filesystem crates in this layer.

No new domain components. The GUI thread calls `domain::latency::measure_latency` ([mvp-01](mvp-01-latency-measurement-primitive.md)) directly with a copy of the captured samples — domain is the inner ring, callable from any layer (rule A3).

### 4.2 Audio adapter — realtime shell

> Rule A2 reminder: **no allocation, no locking, no syscall on the per-buffer path**.

```diff
src/audio/latency.rs (new module)
├── 🟢 ⚙️ LatencyMeter                                                                      src/audio/latency.rs
+      Implements `Process`. Holds shared state with the GUI via Arcs; per-buffer body is
+      atomic stores + tight numeric loop — A2-safe. Captures the inbound buffer into atomic
+      slots; while emit_impulse_pending, overwrites the buffer with a Kronecker impulse on
+      the first frame of the capture window.
│   ├── 🟢 🎯 capture_buffer: Arc<[AtomicU32; 4096]>
+              f32 stored via to_bits()/from_bits(). One alloc at Default.
│   ├── 🟢 🎯 state: Arc<AtomicU8>                                                          (CaptureState as u8)
│   ├── 🟢 🎯 arm_request: Arc<AtomicBool>                                                  (GUI sets; audio swaps to false)
│   ├── 🟢 🎯 write_idx: usize                                                               (audio-thread local)
│   └── 🟢 🎯 emit_impulse_pending: bool                                                     (audio-thread local)
└── 🟢 🪝 LatencyHandle                                                                      src/audio/latency.rs
+      Cloned Arcs + safe accessors handed to the editor for poll/read/reset/request.

src/audio/plugin.rs
├── 🟡 📡 TonismPlugin                                                                      src/audio/plugin.rs
+      Add `latency_meter: LatencyMeter` field; pass `latency_meter.handle()` to editor.
│   ├── 🟡 📡 TonismPlugin::process()                                                       src/audio/plugin.rs:166–213
+          Call `self.latency_meter.process(channel)` on channel 0 only, between Stage 1
+          and Stage 2. Other channels skip the meter. (See "Per-channel handling" below.)
+          When bypass transitions on while state == Capturing: call `self.latency_meter.cancel()`.
│   └── 🟡 📡 TonismPlugin::initialize()                                                    src/audio/plugin.rs:121–135
+          Forward `prepare(sr, max_block_size)` to `latency_meter` alongside `gain_block`.
│   ├── 🟡 📡 TonismPlugin::reset()                                                         src/audio/plugin.rs:139–141
+          Forward to `latency_meter.reset()`.
└── ⚪ 📦 TonismParams                                                                       src/audio/params.rs
+      No change — trigger is via Arc<AtomicBool>, not a BoolParam.
```

#### `LatencyMeter` lifecycle and contract

```rust
// Sketch only — exact API settled at code time.
pub struct LatencyMeter {
    capture_buffer: Arc<[AtomicU32; CAPTURE_LEN]>,
    state: Arc<AtomicU8>,
    arm_request: Arc<AtomicBool>,
    write_idx: usize,
    emit_impulse_pending: bool,
}

impl Process for LatencyMeter {
    fn prepare(&mut self, _sr: SampleRate, _max_block_size: usize) {
        // No-op: capture buffer is fixed-size, allocated in Default.
    }
    fn reset(&mut self) {
        self.write_idx = 0;
        self.emit_impulse_pending = false;
        self.state.store(CaptureState::Idle as u8, Release);
    }
    fn process(&mut self, buffer: &mut [f32]) {
        // 1. On entry: check arm_request swap-to-false; if was true and state == Idle,
        //    transition to Capturing, write_idx = 0, emit_impulse_pending = true.
        // 2. Per sample (only while state == Capturing && write_idx < CAPTURE_LEN):
        //    - capture_buffer[write_idx].store(sample.to_bits(), Release)
        //    - if emit_impulse_pending: sample = if write_idx == 0 { 1.0 } else { 0.0 };
        //      after first sample written, emit_impulse_pending = false.
        //    - write_idx += 1
        //    - if write_idx == CAPTURE_LEN: state.store(Done, Release)
        // 3. While state != Capturing, leave samples untouched.
    }
}

impl LatencyMeter {
    pub fn arm(&mut self) {
        self.arm_request.store(true, Release);
    }
    pub fn cancel(&mut self) {
        // Audio-thread call when bypass flips on mid-capture.
        if self.state.load(Acquire) == CaptureState::Capturing as u8 {
            self.state.store(CaptureState::Done as u8, Release);
            // GUI sees Done with a Cancelled marker (write_idx < CAPTURE_LEN).
        }
    }
    pub fn handle(&self) -> LatencyHandle { /* clone Arcs */ }
}
```

#### Per-channel handling

`Process::process(&mut [f32])` is per-channel. `TonismPlugin::process` iterates channels with `buffer.as_slice().iter_mut().enumerate()` and calls `latency_meter.process(channel)` only when `idx == 0`. The impulse is emitted on channel 0; the captured signal is channel 0's loopback. Multi-channel symmetry is a v0.2 concern.

#### Realtime constraints checklist (`LatencyMeter::process`)

- [x] No `Vec::push`, `Box::new`, or other heap allocation. The capture buffer is `Box<[AtomicU32; CAPTURE_LEN]>` allocated in `Default`.
- [x] No `Mutex`, `RwLock`. Only `AtomicU8`, `AtomicU32`, `AtomicBool`.
- [x] No filesystem, network, or `println!` calls.
- [x] All scratch buffers pre-allocated. The capture buffer holds the only pre-alloc; no per-call scratch.
- [x] No logging on the capture path. (mvp-03 logs xruns only.)

### 4.3 Control surface — GUI / MIDI

```diff
src/gui/editor.rs
├── 🟡 🪟 create()                                                                          src/gui/editor.rs:45–91
+      Add `latency_handle: LatencyHandle` parameter alongside `xrun_counter`. Replace static
+      "latency: -- ms" Label (line 88) with:
+      - A `Button` labeled "Measure latency" wired to `latency_handle.request_measurement()`.
+      - A live Label driven by Memo<LatencyDisplay> updated by the existing 16 ms Timer
+        (the Timer that polls XrunCounter — extend its body to also poll the LatencyHandle).
│   ├── 🟢 🎛️ measure_button                                                                src/gui/editor.rs
+          Button::new(...).on_press(move |_cx| { latency_handle.request_measurement(); })
+          Press → audio thread reads arm_request next buffer and arms the meter.
│   ├── 🟢 🪝 latency_signal: SyncSignal<LatencyDisplay>                                     src/gui/editor.rs
+          Updated by the existing 60 Hz Timer.
+          Body extension: read latency_handle.state(); on transition to Done, call
+          `latency_handle.read_capture_into(&mut local_buf)` (capacity-checked Vec built
+          once outside the closure scope), call `domain::latency::measure_latency(
+          &KRONECKER_REF, &local_buf, sample_rate)`, map Result→LatencyDisplay, set the
+          signal, call `latency_handle.reset_to_idle()`.
│   └── 🟢 🌍 LatencyDisplay → format string                                                 src/gui/editor.rs
+          Memo formats per D3.
└── ⚪ 🚇 ParamSlider, VStack, Label, Button                                                  (vizia_plug widgets)
+      Reused.
```

The reference impulse for cross-correlation lives as a const in `src/audio/latency.rs`:

```rust
pub const KRONECKER_REF: [f32; 1024] = {
    let mut a = [0.0_f32; 1024];
    a[0] = 1.0;
    a
};
```

Both audio thread (emits) and GUI thread (compares) reference this const; no shared state about "what was emitted". The const fits in one cache line's worth of constant data; embedded in the binary.

#### Routing / navigation

No new panel. The existing `VStack` in [editor.rs:68–89](../../../../src/gui/editor.rs) is amended.

### 4.4 Persistence

No persistence change.

### 4.5 Composition root

🟡 [src/audio/plugin.rs](../../../../src/audio/plugin.rs):
- Add `latency_meter: LatencyMeter` field to `TonismPlugin` (alongside `gain_block`).
- Construct in `Default`: `latency_meter: LatencyMeter::default()`.
- Pass the handle to the editor: `crate::gui::editor::create(self.params.clone(), self.xrun_counter.clone(), self.latency_meter.handle())` (line 116).

🟡 [src/audio/mod.rs](../../../../src/audio/mod.rs): add `pub mod latency;`.

### 4.6 Key technical decisions

- **`AtomicU32` for f32 samples** — `f32::to_bits` / `f32::from_bits` is a zero-cost reinterpret; `AtomicU32::store(Release)` + `load(Acquire)` gives a wait-free SPSC-shaped channel without an actual queue. Standard pattern in audio code.
- **State machine is `AtomicU8`, not a typed `Atomic<CaptureState>`** — Rust `std` does not ship `AtomicEnum`; `AtomicU8` + `repr(u8)` round-trip is the idiomatic substitute.
- **Capture length fixed at 4096** — covers the AC bar + headroom. Variable-length capture would force a heap path or a recompile per SR; neither is justified.
- **Capture only channel 0** — multi-channel divergence is a v0.2 concern.
- **Trigger via `Arc<AtomicBool>`, not a `BoolParam`** — keeps the GUI-shared interface fully self-contained inside `LatencyMeter`/`LatencyHandle` and mirrors the [`XrunCounter`](../../../../src/audio/xrun.rs) pattern. The nih-plug param system stays reserved for parameters the host should automate (gain, bypass) rather than for one-shot UI commands.
- **`LatencyMeter` lives in `src/audio/`, not `src/domain/`** — it holds GUI-shared `Arc<Atomic*>` state. Domain stays free of concurrency-shared state per rule A1; the meter is a shell-block that *implements* the domain `Process` trait, which is the legal direction (shell depends on domain, not the reverse — rule A3).
- **Bypass-cancel discipline** — when `bypass.value()` transitions `false → true` in `TonismPlugin::process` and `latency_meter.state == Capturing`, the audio thread calls `latency_meter.cancel()`. The GUI detects partial capture (write_idx < CAPTURE_LEN at Done) and renders `LatencyDisplay::Cancelled`.

### 4.7 Justification of deviation from standards

None. Hexagonal-clean: domain (`measure_latency`) is pure; audio adapter holds the realtime state machine; control surface holds the widget. Messaging is one-way (rule F4): GUI sets atomics, audio thread snapshots into atomics that the GUI polls. The block sits inside the chain alongside `Gain` per rule A4 (external dependencies via traits in the domain) — `Process` is the trait, `LatencyMeter` is a shell-side impl.

---

## 5. Tests

### 5.1 e2e (audio path) — Testing Trophy: thin but real

The story's Manual validation checklist *is* the e2e for AC1 acceptance. No automated e2e is possible without a real audio interface in CI. Codified in the spec's verification protocol.

### 5.2 Integration (domain + adapter) — load-bearing

- 🟢 `tests/latency_meter_round_trip.rs::round_trip_with_synthetic_delay_via_buffer_backend`
  - Build a synthetic input vec: `zeros(delay) + impulse(1024) + zeros(...)` (total length ≥ CAPTURE_LEN).
  - Build a `LatencyMeter` directly. Call `meter.arm()`. Drive it via [`BufferBackend`](../../../../src/audio/backend.rs:33) — exactly the same pattern as [smoke.rs](../../../../tests/smoke.rs) tests `Gain`.
  - After the run, read the capture buffer via `meter.handle().read_capture_into(&mut captured)`, call `domain::latency::measure_latency(&KRONECKER_REF, &captured, SampleRate::new(48_000.0))`, assert the recovered delay matches.
  - Repeats over [`BUFFER_SIZES`](../../../../tests/common/fixtures.rs:8) to verify the state machine survives any buffer chunking.
- 🟢 `tests/latency_meter_round_trip.rs::silent_loopback_yields_no_signal`
  - Same harness with silent input → assert `measure_latency` returns `Err(DomainError::LatencyNoPeak)`.

This integration test is what Option O buys over Option A: `LatencyMeter` is a `Process` impl, so `BufferBackend` drives it without any TonismPlugin glue.

### 5.3 Unit (audio shell) — co-located in `src/audio/latency.rs`

- 🟢 `meter_idle_with_no_arm_passes_buffer_unchanged` — `process()` while state == Idle leaves the input buffer untouched and write_idx == 0.
- 🟢 `meter_arm_transitions_idle_to_capturing` — call `arm()`; first `process()` reads the arm_request swap-to-false and transitions.
- 🟢 `meter_emits_impulse_on_first_frame_of_capture_window` — after `arm()`, the first sample of the next `process` call is overwritten to 1.0 and subsequent samples to 0.0 within the capture window.
- 🟢 `meter_captures_input_before_overwriting` — the capture buffer at index 0 stores the original input sample (not 1.0).
- 🟢 `meter_completes_after_capture_len_samples` — drive `process` calls until total samples == CAPTURE_LEN; assert `state == Done`.
- 🟢 `meter_arm_while_capturing_is_ignored` — arm twice; second arm does not restart capture.
- 🟢 `meter_cancel_during_capture_transitions_to_done` — arm; partial process; call `cancel()`; assert `state == Done` with write_idx < CAPTURE_LEN.
- 🟢 `meter_alloc_free_under_debug_assert_no_alloc` — wrap a `process()` call sequence under the cargo feature; assert no panic.

`LatencyHandle`:
- 🟢 `handle_request_measurement_sets_arm_request` — atomic round-trip via the handle.
- 🟢 `handle_read_capture_into_copies_full_buffer_when_done` — fill the buffer, set state Done, read back; assert equality.
- 🟢 `handle_reset_to_idle_only_transitions_from_done` — Idle / Capturing CAS no-op; Done → Idle succeeds.

`LatencyDisplay`:
- 🟢 `latency_display_format_table` — co-located in [`src/gui/editor.rs`](../../../../src/gui/editor.rs) `mod tests`. Verifies the `LatencyDisplay → String` mapping for all 5 variants per D3.

### 5.4 AC coverage table

| AC / Checklist item                                 | Test                                                                  |
| --------------------------------------------------- | --------------------------------------------------------------------- |
| Click → readout shows within 2 s, < 10 ms (loopback)| **manual** (verification protocol)                                    |
| Audio thread emits impulse + captures N samples     | unit `meter_emits_impulse_on_first_frame_of_capture_window` + `meter_completes_after_capture_len_samples` |
| Captured input is the loopback, not the impulse     | unit `meter_captures_input_before_overwriting`                        |
| End-to-end recovery of synthetic delay              | integration `round_trip_with_synthetic_delay_via_buffer_backend`      |
| Readout persists between clicks                     | reading review (LatencyDisplay::Measured held in SyncSignal)          |
| Bypass during measurement → clean cancel            | unit `meter_cancel_during_capture_transitions_to_done`                |
| Silent input → "no signal" sentinel                 | integration `silent_loopback_yields_no_signal` + `LatencyDisplay::NoSignal` mapping |
| Audio device fails → no crash, label unchanged      | **manual** (nih-plug error path is not under test)                    |
| Second click while measuring → ignored              | unit `meter_arm_while_capturing_is_ignored`                           |
| Status: buffer size / SR / backend / OS visible     | reading review of the `tracing::info!` on `initialize`                |
| `--features debug-assert-no-alloc` → no panic       | unit `meter_alloc_free_under_debug_assert_no_alloc`                   |

---

## 6. Dependencies and execution order

- **Prerequisite stories**: [mvp-01](mvp-01-latency-measurement-primitive.md) — `domain::latency::{measure_latency, LatencyMs}` and the new `DomainError` variants must exist.
- **Stories unblocked**: AC1 verification can run once this lands. mvp-04 / mvp-05 may opt into adding `LatencyMeter` to their stress harness as a follow-up; not required for those stories to land.
- **Commands to run** (local + CI):
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test latency_meter`
  - `cargo test --features debug-assert-no-alloc latency_meter`
  - **Manual**: `cargo run --release` with output→input loopback cabled at the dev interface; click "Measure latency"; expect a value in ms.

---

## 7. Risks and open questions

- 🟢 **D1 (architecture) and D2 (synchronisation primitive) resolved** — see "Decisions taken" above.
- 🟡 **One-shot button ergonomics in vizia 0.4.0** — the `Button.on_press` closure is verified to fire on the GUI thread; the closure needs to capture the `LatencyHandle` (an `Arc` clone, cheap). Verify in code that `vizia_plug`'s `Button::new` accepts `move |cx| { ... }` closures with captured `Arc`s. If not, fall back to a vizia `Element` with a custom `on_press` event.
- 🟡 **Loopback signal level** — the threshold in `measure_latency` rejects very quiet captures. If the dev interface has high attenuation between output and input, the silent-input sentinel may fire even with a real cable. Tune the threshold or boost output gain in the protocol.
- 🟢 **Existing scaffold pays off** — `XrunCounter`/`SyncSignal`/`Memo`/`ParamSlider` give a complete template for atomic state + 60 Hz GUI poll + reactive readout; `Gain` gives the `Process`-block + `BufferBackend` test pattern; the `rtrb` log bridge gives a precedent for shared-Arc audio-side state. mvp-02 reuses every one.
- 🟢 **Block pattern pays back v0.2** — `LatencyMeter` is the second `Process` block in the codebase (after `Gain`). v0.2's multi-block chain inherits the integration-test pattern (`BufferBackend` driving any `Process` impl) and the GUI-shared-Arc pattern (block holds `Arc<Atomic*>`, exposes a handle to the editor).

---

## 8. References

- Similar implementations to follow: `XrunCounter` atomic + GUI poll ([xrun.rs](../../../../src/audio/xrun.rs) + [editor.rs:49–85](../../../../src/gui/editor.rs)); `audio_logger` rtrb pattern ([log_bridge.rs](../../../../src/audio/log_bridge.rs)); `Gain` block + `BufferBackend` integration test ([smoke.rs](../../../../tests/smoke.rs)); existing per-frame `process` body in [plugin.rs:166–213](../../../../src/audio/plugin.rs).
- Directly applicable standards: [architecture.md](../../../standards/architecture.md) (A1, A2, A3, A4, A6, F4), [domain.md](../../../standards/domain.md) (E4), [testing.md](../../../standards/testing.md) (G3, G4, G5), [infrastructure.md](../../../standards/infrastructure.md) (J2 — handled by mvp-03 for the audio→log bridge).
- Related ADRs: [ADR-002](../../../adr/002-standalone-runner.md), [ADR-003](../../../adr/003-gui-library.md).
- Source spec section: [acceptance criteria AC1](../spec.md#acceptance-criteria); [dependencies — latency measurement](../dependencies.md#patterns).
