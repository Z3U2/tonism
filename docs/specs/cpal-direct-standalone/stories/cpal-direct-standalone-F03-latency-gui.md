# cpal-direct-standalone-F03 — Latency measurement button and readout in standalone GUI

**As a** gigging guitarist, **in** the standalone window, **when** I click "Measure latency", **then** the app emits an impulse, captures the loopback, computes round-trip latency, and displays the result in milliseconds — or shows "no signal" if no loopback path exists.

> Derived from spec: [spec.md](../spec.md) — Phase F

## Functional description

This story ports the `LatencyDisplay` state machine and "Measure latency" button from `src/gui/editor.rs` into `src/gui/standalone.rs`. The standalone `TonismApp` accepts a `LatencyHandle` (from F02) and a shared sample-rate value (`Arc<AtomicU32>`) so `measure_latency()` uses the actual session rate instead of the hard-coded 48 kHz. The per-frame poll reads `latency_handle.state()`, transitions the display, and on `Done` copies the capture buffer and calls `measure_latency()`.

Diagnostic logging (`log_capture_diagnostics`) is included so failed measurements are debuggable.

Layers touched: Control surface.

## Acceptance criteria

### Success scenarios

- "Measure latency" button is visible in the standalone window below the xrun counter.
- Clicking the button while state is Idle arms the meter; the label transitions to "latency: measuring...".
- When capture completes (Done), the label shows "latency: X.X ms" where X.X is the computed value.
- Clicking again after a result re-arms the meter (reset_to_idle → new measurement).
- The readout persists between frames until the next measurement is triggered.

### Failure scenarios

- No loopback cable / no signal: label shows "latency: no signal". No crash, no panic.
- Bypass toggled mid-measurement: label shows "latency: -- ms" (Cancelled state). User can re-arm.
- If sample rate is not yet known (zero, which shouldn't happen after stream start): function returns InvalidSampleRate, displayed as "no signal" gracefully.

## Manual validation checklist

- [ ] `cargo run` — "Measure latency" button visible below separator.
- [ ] With a loopback cable (or aggregate device routing output→input): click button, confirm "measuring..." then a numeric ms result appears.
- [ ] Without loopback: click button, confirm "no signal" appears (no crash).
- [ ] Toggle bypass while measuring: confirm "-- ms" (cancelled) appears.
- [ ] Click button again after any result: confirm new measurement starts.
- [ ] Verify the displayed sample rate matches the device config printed at startup.
