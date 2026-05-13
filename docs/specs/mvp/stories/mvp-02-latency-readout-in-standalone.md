# mvp-02 — Latency readout in the standalone

**As a** the author preparing the dev rig before a verification session, **in** the standalone window, **when** clicking the "Measure latency" control with the audio output cabled back into the audio input (loopback), **then** the latency label shows the measured round-trip delay in milliseconds, replacing the `latency: -- ms` placeholder.

> Derived from spec: [MVP — Glitch-free real-time guitar signal path](../spec.md)

## Functional description

Replace the static latency placeholder in the editor with a live readout driven by a real measurement. A user-initiated trigger (button or toggle) emits a single Kronecker impulse on the output for one buffer, captures the inbound loopback into a preallocated buffer for a fixed window, runs the primitive from mvp-01, and pushes the result back to the GUI for display. The audio callback must remain allocation/lock/syscall-free on the per-buffer path — capture buffer is preallocated at `prepare()`, trigger and completion are exchanged via atomics or an existing nih-plug param. ⚠️ TR required to pin down the trigger/done state machine and the capture-buffer ownership.

Layers touched: Capture + Render + Control surface.

## Acceptance criteria

### Success scenarios

- With output→input cabled in loopback at the dev sample rate, clicking "Measure latency" shows a numeric ms reading within 2 seconds; the value is < 10 ms on the dev machine and stays stable across repeated clicks.
- The readout updates after each new measurement; without a click, the value persists (does not flicker back to `--`).
- Toggling bypass while a measurement is not running has no effect on the displayed value; toggling bypass during a measurement either cleanly cancels it or completes it correctly (TR to choose).

### Failure scenarios

- With no loopback connected (silent input), the readout shows a clear sentinel like `latency: no signal` instead of `0.0 ms` or a stale value.
- An audio device that fails to start surfaces the existing nih-plug error path; the latency label remains in its placeholder state, no crash.
- Triggering a second measurement while one is already in progress is either ignored or queued — never causes audio dropout or overlapping captures.

## Manual validation checklist

- [ ] Launch `cargo run --release` with headphones connected; cable the audio output back into the audio input on the dev interface.
- [ ] In the standalone window, click "Measure latency" — within ~2 seconds the label shows a value in ms.
- [ ] Note the buffer size, sample rate, audio backend, and OS shown in the status area or terminal logs (this is the "method" record required by AC1).
- [ ] Verify the value is below 10 ms; if not, record the achievable number per the spec's risk-mitigation note.
- [ ] Disconnect the loopback cable, click "Measure latency" again — readout switches to a no-signal sentinel, no crash.
- [ ] Toggle bypass and adjust input/output gain knobs during normal playback (no measurement running) — audio still passes, no glitch, no panic.
- [ ] Run with `--features debug-assert-no-alloc` and trigger a measurement — no allocation panic from the audio callback.
