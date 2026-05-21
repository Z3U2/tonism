# cpal-direct standalone — WIP log

**Status (21/05/2026):** Phase E merged. Phase F stories written — see [stories.md](stories.md).

## Phases completed

| Phase                              | PR                                                          | What landed                                                                                                                                                                                                                                                                                                                                | Verification                                                                                                                                              |
| ---------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A — walking skeleton**           | [#8](https://github.com/Z3U2/tonism/pull/8) (merged)        | `src/bin/feedback.rs` near-copy of cpal `examples/feedback.rs`; default devices; 150 ms latency ring; `rtrb` instead of `ringbuf`; stdin-blocked for the 5-min session.                                                                                                                                                                    | 5-min clean audio on mac + Windows.                                                                                                                       |
| **B — domain in callback**         | [#9](https://github.com/Z3U2/tonism/pull/9) (merged)        | `Gain::process` wired into the cpal output callback after the ring drain. `for sample in data.iter_mut()` (vs consuming `for sample in data`) so the slice survives the `gain_block.process(data)` call.                                                                                                                                  | 5-min clean audio on mac + Windows.                                                                                                                       |
| **C — params + smoothing + C9 + C10** | [#10](https://github.com/Z3U2/tonism/pull/10) (merged)   | New: `src/params.rs` (lock-free `FloatParam` handle + `SmoothedFloatParam` audio reader; `BoolParam`), `src/domain/smoother.rs` (`LinearSmoother` + 9 tests), `src/cpal_direct.rs` (extracted entry + per-frame in/out gain + `--ramp` test harness). C10: `plugin-export` feature-gate of the nih-plug surface. C9: `assert_no_alloc` wrap around cpal callbacks. | Mac + Windows clean baseline; `--ramp` audibly smooth (1 s smoothing, [-18,-6,0,-6] dB cycle); `--features debug-assert-no-alloc -- --ramp` no alloc panic. |
| **D — egui window + audio coexistence** | [#11](https://github.com/Z3U2/tonism/pull/11) (merged) | `eframe` + `egui` always-on deps; `src/gui/standalone.rs` (static `TonismApp` via `eframe::App`); `cpal_direct::run_gui()` opens eframe window alongside running streams; `cpal_direct::run()` stays headless for `feedback` binary; audio setup extracted into shared `setup_audio()` helper. ADR-006 committed. | 5-min clean audio on Mac + Windows with window open + repainting at 60 Hz. |
| **E — GUI ↔ audio param writes wired** | [#12](https://github.com/Z3U2/tonism/pull/12) (merged) | `TonismApp` accepts `TonismParams` + `XrunCounter`; sliders call `FloatParamHandle::set()` on change; checkboxes call `BoolParam::set()` on change; xrun label reads `XrunCounter::read()` each frame. `XrunCounter` instantiated in `setup_audio()`, cloned into both callbacks, bumped on ring over/underflow. | 5-min clean audio on Mac with active slider movement. No xrun increments, no audible artifacts. |

## Decisions locked

- **C10** — nih-plug surface is feature-gated behind `plugin-export`. Default
  build excludes `nih_plug`, `nih_plug_egui`, `egui`, `src/audio/{params,plugin}.rs`,
  `src/gui/editor.rs`. Decision recorded in [spec.md C10 section](spec.md#c10--dormant-plugin-impl).
- **ADR-006** — GUI library for cpal-direct is **`egui` via `eframe` directly**
  (no `nih_plug_egui` adapter for standalone). v0.2+ vision committed to
  ToneStack-style flat UI; egui↔vizia swap stays reversible via rule A4 if
  that vision later shifts. Committed in PR #11.
- **`assert_no_alloc` opt-out of `disable_release`** — fix-up commit on PR #10.
  Without this, the C9 guard becomes a no-op in `--release`, exactly the mode
  realtime audio dev runs in.

## Open follow-ups

- **CI gate for `--features plugin-export`** — spec calls for both feature
  configurations in CI so the dormant `Plugin` impl can't silently drift.
  Small GHA workflow PR, not yet open.
- **`bypass` + `test_signal` not yet read by the cpal callback** — constructed
  in `TonismParams::new()` but only `input_gain` + `output_gain` are wired
  through. Phase F re-integrates these alongside the sine generator +
  latency meter.

## Phase F — stories

User stories for Phase F are split into three deliverables — see [stories.md](stories.md) for the full index:

| ID  | Title                                          | Layers          | Size |
| --- | ---------------------------------------------- | --------------- | ---- |
| F01 | Bypass toggle and 440 Hz test-signal           | Capture+Render  | S    |
| F02 | LatencyMeter wired into cpal output callback   | Capture+Render  | S    |
| F03 | Latency measurement button and readout in GUI  | Control surface | S    |

Dependency chain: F01 → F02 → F03.

Key design decision (deinterleave): pre-allocated scratch buffer extracts channel 0 from interleaved cpal output, runs `LatencyMeter::process`, writes back. No domain-layer changes. See TR notes in stories.md.

## Phases ahead

| Phase | What lands                                                                                                                                                                                                                                |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **G** | Device picker UX + persisted last-used `(input, output, SR, buffer size)` to the OS-conventional config location.                                                                                                                          |
| **H** | Cutover: remove the `nih_export_standalone!` runtime use (currently gated on `plugin-export`); update `docs/standards/architecture.md` "pending decisions" section (composition-root location + lock-free GUI↔audio primitive — both now resolved). |

## Verification baseline (last run: 21/05/2026)

- **Default features** — 50 tests pass (45 lib + 2 integration + 3 smoke; 1 doctest ignored). `eframe` + `egui` in dep graph; `nih_plug` absent.
- **`--features plugin-export`** — 51 tests pass (one extra in the plugin-export-gated surface). `nih_plug` surface compiles.
- **`--features debug-assert-no-alloc`** — clean; `AllocDisabler` live in release thanks to the `default-features = false` opt-out.
- **`--features plugin-export,debug-assert-no-alloc`** — clean; our global allocator yields to nih-plug's via the `not(feature = "plugin-export")` clause in `src/main.rs` and `src/bin/feedback.rs`.
- **`cargo run`** — eframe window opens alongside running cpal audio streams; sliders and checkboxes wired to audio params via lock-free atomics. Xrun counter displays live.
- **`cargo run --bin feedback`** — headless stdin-blocking path unchanged.

## References

- [ADR-005](../../adr/005-standalone-audio-cpal-direct.md) — pivot decision (partially supersedes ADR-002)
- [ADR-006](../../adr/006-gui-library-after-cpal-direct.md) — GUI library re-evaluation
- [spec.md](spec.md) — full implementation spec (components C1–C10, phases A–H)
