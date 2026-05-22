# Implementation plan: cpal-direct-standalone-F03 — Latency measurement button and readout in GUI

**Story**: [cpal-direct-standalone-F03-latency-gui.md](cpal-direct-standalone-F03-latency-gui.md)
**Spec**: [spec.md](../spec.md)
**Layers**: ~~Capture~~ · ~~Signal chain~~ · ~~Render~~ · ~~Tone state~~ · Control surface · ~~Persistence~~ · Tests
**Complexity**: 🟢 Low

---

## 1. Summary

Port the latency-measurement GUI state machine from `src/gui/editor.rs` into `src/gui/standalone.rs`. The `TonismApp` gains a `LatencyHandle` (from F02's `AudioSession`), a shared sample-rate value (`Arc<AtomicU32>`), and a `LatencyEditorState` that drives the "Measure latency" button and readout label. This is a transliteration of existing, tested GUI logic — not a design task.

---

## 2. Functional scope

### Mapping acceptance criteria → components

| Criterion / Case | Layer | Main component | Status |
| --- | --- | --- | --- |
| "Measure latency" button visible below xrun counter | Control surface | `src/gui/standalone.rs` ui() | 🟡 |
| Click → Idle arms meter, label shows "measuring..." | Control surface | `src/gui/standalone.rs` state machine | 🟡 |
| Done → label shows "X.X ms" | Control surface | `src/gui/standalone.rs` + `measure_latency()` | 🟡 |
| Click again re-arms (reset_to_idle → new measurement) | Control surface | `src/gui/standalone.rs` state machine | 🟡 |
| Readout persists between frames | Control surface | `LatencyEditorState.display` field | 🟢 |
| No loopback → "no signal" | Control surface | `src/gui/standalone.rs` (Err → NoSignal) | 🟡 |
| Bypass mid-measurement → "-- ms" (Cancelled) | Control surface | `src/gui/standalone.rs` state machine | 🟡 |
| SR shared with GUI for accurate measurement | Composition root | `Arc<AtomicU32>` in `setup_audio()` + `TonismApp` | 🟢 |

### Out of scope (declared)

- Diagnostic logging via `tracing` — the `log_capture_diagnostics` function uses `tracing::info!`. If `tracing` is not yet in the dep graph for the standalone binary, use `eprintln!` as a stand-in (matching the existing xrun logging pattern). The function shape is identical either way.

---

## 3. Domain & data model

No domain change. No new types.

Reused as-is:
- ⚪ `measure_latency` — `src/domain/latency.rs`
- ⚪ `DEFAULT_MIN_LAG_SAMPLES` — `src/domain/latency.rs`
- ⚪ `LatencyMs` — `src/domain/latency.rs`
- ⚪ `SampleRate` — `src/domain/types.rs`
- ⚪ `CaptureState` — `src/audio/latency.rs`
- ⚪ `LatencyHandle` — `src/audio/latency.rs`
- ⚪ `CAPTURE_LEN`, `N_IMPULSES` — `src/audio/latency.rs`

No persistence change.

---

## 4. Architecture

### 4.1 Domain — pure core

No domain changes.

---

### 4.2 Audio adapter — realtime shell

No audio adapter change in this story (F02 already wired the meter).

One composition-root change: `setup_audio()` must also produce the session sample rate for the GUI. See 4.5.

---

### 4.3 Control surface — GUI

#### Components

```diff
src/gui/standalone.rs
├── 🟢 LatencyDisplay enum
│   Variants: Pending, Measuring, Measured(f32), NoSignal, Cancelled.
│   Method: format() -> String.
│   Port of src/gui/editor.rs lines 20–42. Identical shape.
│
├── 🟢 LatencyState struct (GUI-thread-only)
│   Fields: display: LatencyDisplay, capture_buf: Vec<f32>.
│   Pre-allocates capture_buf to CAPTURE_LEN capacity.
│   Port of src/gui/editor.rs LatencyEditorState (lines 51–62).
│
├── 🟡 TonismApp struct (lines 15–18)
│   + latency_handle: LatencyHandle
│   + sample_rate: Arc<AtomicU32>
│   + latency_state: LatencyState
│
├── 🟡 TonismApp::new() (lines 21–31)
│   + Accept latency_handle: LatencyHandle, sample_rate: Arc<AtomicU32>
│   + Initialize latency_state: LatencyState::default()
│
├── 🟡 TonismApp::ui() (lines 35–70)
│   Replace the "latency: -- ms" placeholder (line 67) with:
│   1. Per-frame state machine (match self.latency_handle.state())
│      - Idle: leave display unchanged
│      - Capturing: set display = Measuring
│      - Done: read_capture_into, log diagnostics, call measure_latency, set Measured/NoSignal, reset_to_idle
│      - Cancelled: set display = Cancelled, reset_to_idle
│   2. Button: ui.button("Measure latency") → .request_measurement()
│   3. Label: ui.label(self.latency_state.display.format())
│
└── 🟢 log_capture_diagnostics(capture: &[f32], sr: f32) — free function
    Port of src/gui/editor.rs lines 204–227.
    Uses eprintln! (or tracing::info! if available).
```

#### Data flow

```
GUI thread (60 Hz repaint via request_repaint_after(16 ms))
    │
    ├─ latency_handle.state() → CaptureState (atomic read)
    │   - Idle: no-op
    │   - Capturing: display = Measuring
    │   - Done:
    │     ├─ latency_handle.read_capture_into(&mut capture_buf)
    │     ├─ log_capture_diagnostics(&capture_buf, sr)
    │     ├─ measure_latency(&capture_buf, N_IMPULSES, DEFAULT_MIN_LAG_SAMPLES, SampleRate::new(sr))
    │     ├─ display = Measured(ms) or NoSignal
    │     └─ latency_handle.reset_to_idle()
    │   - Cancelled:
    │     ├─ display = Cancelled
    │     └─ latency_handle.reset_to_idle()
    │
    └─ ui.button("Measure latency").clicked() → latency_handle.request_measurement()
```

#### Sample rate sharing

- 🟢 `Arc<AtomicU32>` created in `setup_audio()`, storing `sr.value().to_bits()`.
- Passed to `TonismApp::new()` and read on the GUI thread as `f32::from_bits(arc.load(Relaxed))`.
- Matches the `XrunCounter` pattern (Arc + atomic) already established.
- The SR is set once at stream start and does not change within a session (Phase G will handle device switches). No race condition.

---

### 4.4 Persistence

No persistence change.

---

### 4.5 Composition root

- 🟡 `setup_audio()` in `src/cpal_direct.rs`:
  - 🟢 Create `sample_rate_shared = Arc::new(AtomicU32::new(sr.value().to_bits()))`.
  - Return it alongside `latency_handle` in `AudioSession`.
- 🟡 `AudioSession` struct:
  - + `sample_rate: Arc<AtomicU32>` field (in addition to `latency_handle` from F02).
- 🟡 `run_gui()`:
  - Destructure `latency_handle` and `sample_rate` from `AudioSession`.
  - Pass both to `TonismApp::new(cc, gui_params, xrun_counter, latency_handle, sample_rate)`.

---

### 4.6 Key technical decisions

- **`Arc<AtomicU32>` for sample rate**: the simplest primitive matching the existing project pattern (`XrunCounter`, `BoolParam`, `FloatParamHandle` all use Arc + atomic). The SR is written once at stream setup and read each time the GUI computes a latency value — no contention, no lock.
- **`LatencyDisplay` as a local enum in `standalone.rs`**: not shared with `editor.rs`. The nih-plug editor will eventually be gated behind `plugin-export`; the standalone GUI owns its own copy. If a future refactor wants to share it, it can be moved to a `src/gui/common.rs` module �� but that's not this story.
- **`eprintln!` for diagnostics**: the diagnostic log uses `eprintln!` to match the existing xrun logging pattern in `src/cpal_direct.rs`. This runs on the GUI thread (not the audio thread), so A2 does not apply. If `tracing` is later adopted per infrastructure.md J2, a trivial s/eprintln!/tracing::info!/ migration covers it.

---

## 5. Tests

### 5.1 e2e (audio path)

No e2e impact — this story is GUI-only.

### 5.2 Integration

No new integration test. The latency meter's audio-side behaviour is already covered by F02's tests. The GUI state machine is a direct translation of `editor.rs` which has a unit test for `LatencyDisplay::format()`.

### 5.3 Unit

- 🟢 **`latency_display_format_table`** — unit test in `src/gui/standalone.rs` `#[cfg(test)] mod tests`. Table-driven: assert each `LatencyDisplay` variant produces the expected string. Mirrors the existing test in `src/gui/editor.rs:232–243`.

### 5.4 AC coverage table

| AC / Checklist item | Test |
| --- | --- |
| Button visible | Manual: `cargo run` |
| Click → "measuring..." | Manual: click button |
| Done → "X.X ms" | Manual: loopback cable + click |
| No loopback → "no signal" | Manual: no cable + click |
| Bypass mid-measurement → "-- ms" | Manual: toggle bypass during capture |
| Click again → new measurement | Manual: re-click after result |
| SR matches device config | Manual: compare GUI readout origin with startup println |
| `LatencyDisplay::format()` correctness | Unit test `latency_display_format_table` |

---

## 6. Dependencies and execution order

- **Prerequisite stories**: F02 must land first — provides `LatencyHandle` and `AudioSession.latency_handle`.
- **Stories unblocked**: None within Phase F. Phase G (device picker) is the next phase.
- **Commands to run**:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo run` — manual verification of the full latency-measurement flow

---

## 7. Risks and open questions

- 🟢 **Low risk** — this is a transliteration of existing, working code from `editor.rs` into `standalone.rs`. The state machine, the `measure_latency()` call, and the diagnostics are all proven in the nih-plug path.
- 🟢 **No realtime concerns** — all work happens on the GUI thread. `read_capture_into()` allocates on the first call (growing the Vec to CAPTURE_LEN) but this is on the GUI thread, not the audio thread. Subsequent calls reuse the capacity.
- 🟡 **Sample rate accuracy** — the shared SR is set once from `config.sample_rate` at stream start. If the OS reports an unexpected rate (e.g. 0), `measure_latency` returns `InvalidSampleRate` which maps to "no signal" in the GUI. Not a crash path.
- 🟡 **`tracing` dependency** — if not yet in the dep graph, use `eprintln!`. Switching to `tracing` later is a one-line change per call site. Do not block this story on adopting a logging crate.

---

## 8. References

- Source to port: [`src/gui/editor.rs:20–227`](../../../../src/gui/editor.rs) — `LatencyDisplay`, state machine, `log_capture_diagnostics`.
- Target file: [`src/gui/standalone.rs`](../../../../src/gui/standalone.rs) — current TonismApp.
- LatencyHandle API: [`src/audio/latency.rs:170–230`](../../../../src/audio/latency.rs) — `request_measurement()`, `state()`, `read_capture_into()`, `reset_to_idle()`.
- Domain measurement: [`src/domain/latency.rs:67–137`](../../../../src/domain/latency.rs) — `measure_latency()`.
- Architecture rule F4: [architecture.md](../../../standards/architecture.md) — GUI→audio one-way via lock-free primitive.
