# cpal-direct-standalone-F01 — Bypass toggle and 440 Hz test-signal in cpal-direct path

**As a** gigging guitarist, **in** the standalone window, **when** I toggle the Bypass checkbox, **then** the signal path passes audio through unchanged (no gain, no processing); **when** I toggle the Test Signal checkbox, **then** a 440 Hz sine replaces the mic input so I can verify the path is alive without plugging in.

> Derived from spec: [spec.md](../spec.md) — Phase F

## Functional description

This story wires the two `BoolParam`s (`bypass`, `test_signal`) that currently exist in the param set but are ignored by the cpal-direct callbacks. When bypass is on, the input callback skips the input-gain multiply, and the output callback skips the domain gain block and output-gain multiply — signal passes through the ring unchanged. When test signal is on, the input callback replaces mic samples with a 440 Hz sine (phase accumulator, same shape as `src/audio/plugin.rs` Stage 1) before pushing to the ring.

No allocation, no locking, no syscall on the per-buffer path (rule A2).

Layers touched: Capture, Render.

## Acceptance criteria

### Success scenarios

- Bypass OFF, test signal OFF: signal flows through input_gain → ring → gain_block → output_gain (existing behaviour unchanged).
- Bypass ON: audio passes through unmodified — no gain applied. Toggling bypass back OFF restores processing without a click (gain smoothers are already at their targets).
- Test signal ON, bypass OFF: a clean 440 Hz tone is audible in headphones regardless of whether a mic/guitar is connected. The tone respects the gain trims.
- Test signal ON, bypass ON: the 440 Hz tone passes through unchanged (no gain applied).

### Failure scenarios

- If bypass is toggled during a latency measurement capture (Phase F02), the measurement is cancelled cleanly (LatencyMeter::cancel is called). This criterion is only fully testable after F02 lands, but the `cancel()` call site must be present in this story's bypass path.

## Manual validation checklist

- [ ] `cargo run` — toggle Bypass checkbox, confirm processed gain disappears (output sounds identical to raw input level).
- [ ] Toggle Bypass OFF — confirm gain trims re-engage without audible click.
- [ ] Toggle Test Signal ON — confirm 440 Hz tone audible even without a mic connected.
- [ ] Toggle both ON — confirm 440 Hz tone passes through at unity.
- [ ] `cargo test` — confirm existing tests still pass (no regression).
