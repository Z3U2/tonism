# mvp-03 — Wire xrun detection into the audio callback

**As a** the author monitoring stability during a session, **in** the standalone window, **when** the audio backend or callback path experiences a buffer-underrun condition, **then** the `xrun:` label increments in real time and reflects the cumulative count for the session.

> Derived from spec: [MVP — Glitch-free real-time guitar signal path](../spec.md)

## Functional description

The `XrunCounter` (`Arc<AtomicU64>`) is wired into the GUI via a 60 Hz `SyncSignal` poll already, but the callback in `src/audio/plugin.rs` never increments it because cpal-via-nih-plug standalone does not surface `StreamError::InputUnderflow` to the plugin. This story closes the gap: the TR picks the detection mechanism (wall-clock budget check inside `process()`, a fork of nih-plug exposing the cpal stream-error event, or a parallel raw-cpal hook), and the callback writes the increment plus an event to the existing `rtrb` audio→log bridge — no allocation, no locking, no syscall on the per-buffer path. The dead-code attribute on `audio_logger` comes off as a side-effect. ⚠️ TR required to pick the mechanism.

Layers touched: Capture + Control surface.

## Acceptance criteria

### Success scenarios

- Under normal operating conditions (buffer size ≥ 256, sample rate 48 kHz, idle or moderate input), the `xrun:` label reads `0` and stays at `0` for a 1-minute observation.
- Forcing an underrun (e.g. reducing buffer size below the system's stable floor, or running an artificial sleep stub gated behind a developer-only flag) increments the label within the GUI's poll cadence, and the increment count matches the number of induced events ±1.
- Each xrun increment also produces an entry on the off-RT log drain (`tracing` output via the `rtrb` bridge), making the events visible in the terminal.

### Failure scenarios

- A spurious early-buffer event during stream startup does not get counted as an xrun (or is documented as a known one-time count and filtered).
- Closing the audio stream and reopening it within the same session resets or preserves the counter per the TR's choice — whichever, the behavior is documented in the editor and the log.
- The detection path itself never panics, never allocates, and never blocks the callback — verified with `--features debug-assert-no-alloc` under stress.

## Manual validation checklist

- [ ] Launch `cargo run --release`, plug in guitar + headphones, observe `xrun: 0` for at least 1 minute of light playing.
- [ ] In a separate terminal, run with `--features debug-assert-no-alloc` and a developer-only "force-underrun" toggle (or a buffer size reduced to a known-bad value) — the counter increments as expected, no allocation panic, no crash.
- [ ] Verify the off-RT log shows one tracing event per increment.
- [ ] Restore stable buffer size — counter stops climbing; existing count remains visible per the TR's preserve/reset choice.
- [ ] Toggle bypass and sweep input/output gain during the stress run — counter behavior is independent of parameter changes.
