# cpal-direct standalone — WIP log

**Status (22/05/2026):** Phase F implemented — [PR #13](https://github.com/Z3U2/tonism/pull/13) open. Pending manual 5-min verification.

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
- **Deinterleave for LatencyMeter** — pre-allocated 256 KB scratch buffer
  extracts channel 0 from the interleaved cpal output, runs
  `LatencyMeter::process`, writes back. No domain-layer changes. Decided in
  Phase F (PR #13).

## Open follow-ups

- **CI gate for `--features plugin-export`** — spec calls for both feature
  configurations in CI so the dormant `Plugin` impl can't silently drift.
  Small GHA workflow PR, not yet open.

## Phase F — [PR #13](https://github.com/Z3U2/tonism/pull/13) (open)

All three stories implemented in a single PR:

| ID  | Title                                          | Layers          | Status |
| --- | ---------------------------------------------- | --------------- | ------ |
| F01 | Bypass toggle and 440 Hz test-signal           | Capture+Render  | Done   |
| F02 | LatencyMeter wired into cpal output callback   | Capture+Render  | Done   |
| F03 | Latency measurement button and readout in GUI  | Control surface | Done   |

Additional commits on the PR:
- `disarm()` fix: pending arm requests are cleared under bypass so a stale
  arm doesn't fire when bypass toggles off.
- `--input <name>` / `--output <name>` CLI flags: select audio devices by
  name substring (case-insensitive) without changing system defaults.
  Groundwork for Phase G.

Pending: manual 5-min verification session with loopback cable.

## Phases ahead

| Phase | What lands                                                                                                                                                                                                                                |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **G** | Device picker UX + persisted last-used `(input, output, SR, buffer size)` to the OS-conventional config location.                                                                                                                          |
| **H** | Cutover: remove the `nih_export_standalone!` runtime use (currently gated on `plugin-export`); update `docs/standards/architecture.md` "pending decisions" section (composition-root location + lock-free GUI↔audio primitive — both now resolved). |

## Verification baseline (last run: 22/05/2026)

- **Default features** — 52 tests pass (46 lib + 3 integration + 3 smoke; 1 doctest ignored). `eframe` + `egui` in dep graph; `nih_plug` absent.
- **`--features plugin-export`** — 53 tests pass (one extra in the plugin-export-gated surface). `nih_plug` surface compiles.
- **`--features debug-assert-no-alloc`** — clean; `AllocDisabler` live in release thanks to the `default-features = false` opt-out.
- **`--features plugin-export,debug-assert-no-alloc`** — clean; our global allocator yields to nih-plug's via the `not(feature = "plugin-export")` clause in `src/main.rs` and `src/bin/feedback.rs`.
- **`cargo run`** — eframe window opens; sliders, bypass, test signal, xrun counter, and "Measure latency" button all functional. Latency readout shows ms on loopback, "no signal" without.
- **`cargo run -- --input "BlackHole" --output "BlackHole"`** — device selection by name works.
- **`cargo run --bin feedback`** — headless stdin-blocking path unchanged.

## References

- [ADR-005](../../adr/005-standalone-audio-cpal-direct.md) — pivot decision (partially supersedes ADR-002)
- [ADR-006](../../adr/006-gui-library-after-cpal-direct.md) — GUI library re-evaluation
- [spec.md](spec.md) — full implementation spec (components C1–C10, phases A–H)
