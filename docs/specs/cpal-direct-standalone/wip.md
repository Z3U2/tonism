# cpal-direct standalone — WIP log

**Status (21/05/2026):** Phase D in progress on `feat/cpal-phase-c-params-smoothing`.
Phase C merged. [ADR-006](../../adr/006-gui-library-after-cpal-direct.md) drafted
but not yet committed/PR'd.

## Phases completed

| Phase                              | PR                                                          | What landed                                                                                                                                                                                                                                                                                                                                | Verification                                                                                                                                              |
| ---------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A — walking skeleton**           | [#8](https://github.com/Z3U2/tonism/pull/8) (merged)        | `src/bin/feedback.rs` near-copy of cpal `examples/feedback.rs`; default devices; 150 ms latency ring; `rtrb` instead of `ringbuf`; stdin-blocked for the 5-min session.                                                                                                                                                                    | 5-min clean audio on mac + Windows.                                                                                                                       |
| **B — domain in callback**         | [#9](https://github.com/Z3U2/tonism/pull/9) (merged)        | `Gain::process` wired into the cpal output callback after the ring drain. `for sample in data.iter_mut()` (vs consuming `for sample in data`) so the slice survives the `gain_block.process(data)` call.                                                                                                                                  | 5-min clean audio on mac + Windows.                                                                                                                       |
| **C — params + smoothing + C9 + C10** | [#10](https://github.com/Z3U2/tonism/pull/10) (merged)   | New: `src/params.rs` (lock-free `FloatParam` handle + `SmoothedFloatParam` audio reader; `BoolParam`), `src/domain/smoother.rs` (`LinearSmoother` + 9 tests), `src/cpal_direct.rs` (extracted entry + per-frame in/out gain + `--ramp` test harness). C10: `plugin-export` feature-gate of the nih-plug surface. C9: `assert_no_alloc` wrap around cpal callbacks. | Mac + Windows clean baseline; `--ramp` audibly smooth (1 s smoothing, [-18,-6,0,-6] dB cycle); `--features debug-assert-no-alloc -- --ramp` no alloc panic. |
| **D — egui window + audio coexistence** | (in progress) | `eframe` + `egui` always-on deps; `src/gui/standalone.rs` (static `TonismApp` via `eframe::App`); `cpal_direct::run_gui()` opens eframe window alongside running streams; `cpal_direct::run()` stays headless for `feedback` binary; audio setup extracted into shared `setup_audio()` helper. | Window opens, sliders/checkboxes interactive, audio runs underneath. Pending: 5-min clean-audio verification with window open + repainting. |

## Decisions locked

- **C10** — nih-plug surface is feature-gated behind `plugin-export`. Default
  build excludes `nih_plug`, `nih_plug_egui`, `egui`, `src/audio/{params,plugin}.rs`,
  `src/gui/editor.rs`. Decision recorded in [spec.md C10 section](spec.md#c10--dormant-plugin-impl).
- **ADR-006** — GUI library for cpal-direct is **`egui` via `eframe` directly**
  (no `nih_plug_egui` adapter for standalone). v0.2+ vision committed to
  ToneStack-style flat UI; egui↔vizia swap stays reversible via rule A4 if
  that vision later shifts. **ADR file written, not yet committed/PR'd.**
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
- **ADR-006 still untracked** on the (now-merged) `feat/cpal-phase-c-params-smoothing`
  branch. Needs its own `docs/adr-006-...` branch off main.

## Phases ahead

| Phase | What lands                                                                                                                                                                                                                                |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **D** | egui window stands up next to the running audio stream — no GUI ↔ audio coupling yet. Isolates "adding a window breaks audio" from "GUI traffic breaks audio."                                                                            |
| **E** | GUI → audio param writes wired through `FloatParamHandle::set` / `BoolParam::set`. xrun counter widget reads the existing atomic.                                                                                                         |
| **F** | `LatencyMeter` + test-signal toggle re-integrated. The deinterleave-vs-`Process`-trait question (channel-0-only meter on an interleaved cpal buffer) gets addressed here.                                                                  |
| **G** | Device picker UX + persisted last-used `(input, output, SR, buffer size)` to the OS-conventional config location.                                                                                                                          |
| **H** | Cutover: remove the `nih_export_standalone!` runtime use (currently gated on `plugin-export`); update `docs/standards/architecture.md` "pending decisions" section (composition-root location + lock-free GUI↔audio primitive — both now resolved). |

## Verification baseline (last run: 21/05/2026)

- **Default features** — 50 tests pass (45 lib + 2 integration + 3 smoke; 1 doctest ignored). `eframe` + `egui` in dep graph; `nih_plug` absent.
- **`--features plugin-export`** — 51 tests pass (one extra in the plugin-export-gated surface). `nih_plug` surface compiles.
- **`--features debug-assert-no-alloc`** — clean; `AllocDisabler` live in release thanks to the `default-features = false` opt-out.
- **`--features plugin-export,debug-assert-no-alloc`** — clean; our global allocator yields to nih-plug's via the `not(feature = "plugin-export")` clause in `src/main.rs` and `src/bin/feedback.rs`.
- **`cargo run`** — eframe window opens alongside running cpal audio streams; interactive sliders/checkboxes (not yet wired to audio params).
- **`cargo run --bin feedback`** — headless stdin-blocking path unchanged.

## References

- [ADR-005](../../adr/005-standalone-audio-cpal-direct.md) — pivot decision (partially supersedes ADR-002)
- [ADR-006](../../adr/006-gui-library-after-cpal-direct.md) — GUI library re-evaluation (draft)
- [spec.md](spec.md) — full implementation spec (components C1–C10, phases A–H)
