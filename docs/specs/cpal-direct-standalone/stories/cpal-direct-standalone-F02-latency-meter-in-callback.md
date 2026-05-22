# cpal-direct-standalone-F02 — LatencyMeter wired into the cpal output callback

**As a** gigging guitarist, **in** the standalone app, **when** the latency measurement is armed (by a future GUI button), **then** the audio path emits impulses on channel 0 and captures the loopback return, producing a valid capture buffer for `measure_latency`.

> Derived from spec: [spec.md](../spec.md) — Phase F

## Functional description

This story plumbs the existing `LatencyMeter` (`src/audio/latency.rs`) into the cpal-direct output callback. The meter implements `Process` and expects a mono `&mut [f32]` (channel 0 only). The cpal buffer is interleaved. The solution: a pre-allocated scratch buffer (allocated once during `setup_audio()`, moved into the output closure) that extracts channel 0 from the interleaved output, runs `LatencyMeter::process`, then writes channel 0 back. This is the "deinterleave-vs-Process-trait" question resolved.

The meter sits between the ring drain and the domain gain block — matching the nih-plug signal ordering (input_gain → latency_meter → gain_block → output_gain).

No allocation, no locking, no syscall on the per-buffer path (rule A2). The scratch buffer is a `Vec<f32>` allocated once before stream start and reused via slice indexing.

A `LatencyHandle` is produced in `setup_audio()` and returned in `AudioSession` so the GUI story (F03) can wire the button.

Layers touched: Capture, Render.

## Acceptance criteria

### Success scenarios

- `LatencyMeter` receives channel-0-only samples each callback via the scratch buffer.
- When armed programmatically (via `LatencyHandle::request_measurement()`), the meter transitions Idle → Capturing → Done after `CAPTURE_LEN` (4096) channel-0 samples.
- The capture buffer contains the loopback signal, and `measure_latency()` returns a valid `LatencyMs` value on a loopback device.
- The meter's impulse emission (1.0 at chunk boundaries) appears only on channel 0 of the interleaved output; other channels are unaffected.
- When bypass is on (F01), `LatencyMeter::cancel()` is called, transitioning any in-progress measurement to Cancelled.

### Failure scenarios

- If there is no loopback (open mic, no physical cable), `measure_latency` returns `LatencyNoPeak` — this is expected and handled in F03's GUI.

## Manual validation checklist

- [ ] `cargo test` — new integration test drives `LatencyMeter` through the cpal-direct signal path (synthetic buffer, not real hardware) and asserts Done + valid capture.
- [ ] `cargo run --bin feedback` — headless path still produces clean audio (no regression).
- [ ] `cargo run` — GUI window opens, audio is clean, no xrun increments with the meter idle.
- [ ] Programmatic arm via a temporary `--measure` flag or test harness confirms the meter transitions to Done (removed before merge, or kept as a dev-only flag).
