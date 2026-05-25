# cpal-direct standalone — WIP log

**Status (25/05/2026):** Phase G implemented — PR open. Pending manual verification.

## Phases completed

| Phase                              | PR                                                          | What landed                                                                                                                                                                                                                                                                                                                                | Verification                                                                                                                                              |
| ---------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A — walking skeleton**           | [#8](https://github.com/Z3U2/tonism/pull/8) (merged)        | `src/bin/feedback.rs` near-copy of cpal `examples/feedback.rs`; default devices; 150 ms latency ring; `rtrb` instead of `ringbuf`; stdin-blocked for the 5-min session.                                                                                                                                                                    | 5-min clean audio on mac + Windows.                                                                                                                       |
| **B — domain in callback**         | [#9](https://github.com/Z3U2/tonism/pull/9) (merged)        | `Gain::process` wired into the cpal output callback after the ring drain. `for sample in data.iter_mut()` (vs consuming `for sample in data`) so the slice survives the `gain_block.process(data)` call.                                                                                                                                  | 5-min clean audio on mac + Windows.                                                                                                                       |
| **C — params + smoothing + C9 + C10** | [#10](https://github.com/Z3U2/tonism/pull/10) (merged)   | New: `src/params.rs` (lock-free `FloatParam` handle + `SmoothedFloatParam` audio reader; `BoolParam`), `src/domain/smoother.rs` (`LinearSmoother` + 9 tests), `src/cpal_direct.rs` (extracted entry + per-frame in/out gain + `--ramp` test harness). C10: `plugin-export` feature-gate of the nih-plug surface. C9: `assert_no_alloc` wrap around cpal callbacks. | Mac + Windows clean baseline; `--ramp` audibly smooth (1 s smoothing, [-18,-6,0,-6] dB cycle); `--features debug-assert-no-alloc -- --ramp` no alloc panic. |
| **D — egui window + audio coexistence** | [#11](https://github.com/Z3U2/tonism/pull/11) (merged) | `eframe` + `egui` always-on deps; `src/gui/standalone.rs` (static `TonismApp` via `eframe::App`); `cpal_direct::run_gui()` opens eframe window alongside running streams; `cpal_direct::run()` stays headless for `feedback` binary; audio setup extracted into shared `setup_audio()` helper. ADR-006 committed. | 5-min clean audio on Mac + Windows with window open + repainting at 60 Hz. |
| **E — GUI ↔ audio param writes wired** | [#12](https://github.com/Z3U2/tonism/pull/12) (merged) | `TonismApp` accepts `TonismParams` + `XrunCounter`; sliders call `FloatParamHandle::set()` on change; checkboxes call `BoolParam::set()` on change; xrun label reads `XrunCounter::read()` each frame. `XrunCounter` instantiated in `setup_audio()`, cloned into both callbacks, bumped on ring over/underflow. | 5-min clean audio on Mac with active slider movement. No xrun increments, no audible artifacts. |
| **F — latency meter + bypass + test signal** | [#13](https://github.com/Z3U2/tonism/pull/13) (merged) | Bypass toggle, 440 Hz test-signal injection, LatencyMeter wired into cpal output callback (ch0 deinterleave via scratch buffer), GUI "Measure latency" button + readout, `--input`/`--output` CLI flags. Latency meter: single-impulse design with output muting, ring-delay subtraction. | 5-min clean audio on Mac with BlackHole loopback. Latency measurement consistent at 64 ms (BlackHole 16ch scheduling overhead). |

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
- **DeviceId for config persistence** — cpal's `DeviceId` (stable across reboots
  on CoreAudio/WASAPI) is serialised via `Display`/`FromStr` into the config
  file. Name-based matching kept for CLI `--input`/`--output` flags only.
  Decided in Phase G.
- **confy + serde for config** — persists `TonismConfig` to
  `~/Library/Application Support/tonism/` (macOS) or `%APPDATA%\tonism\`
  (Windows) via `confy` crate. TOML format. Decided in Phase G.

## Open follow-ups

- **CI gate for `--features plugin-export`** — spec calls for both feature
  configurations in CI so the dormant `Plugin` impl can't silently drift.
  Small GHA workflow PR, not yet open.

## Phase G — Device picker UX + config persistence (open)

New modules and refactoring in a single PR:

| ID  | Title                                          | Layers                | Status |
| --- | ---------------------------------------------- | --------------------- | ------ |
| G01 | Config persistence (confy + serde)             | Persistence           | Done   |
| G02 | Device enumeration module                      | Infrastructure (C7)   | Done   |
| G03 | Split setup_audio → create_params + build_streams | Composition root (C8) | Done |
| G04 | GUI device picker + stream lifecycle           | Control surface (C6)  | Done   |
| G05 | wip.md update + verification                   | Docs                  | Done   |

New files:
- `src/config.rs` — `TonismConfig` struct + `load_config()`/`save_config()`
- `src/device.rs` — `DeviceInfo`, `ResolvedDeviceConfig`, `enumerate_devices()`,
  `compute_common_sample_rates()`, `compute_available_buffer_sizes()`,
  `resolve_initial_config()`

Key refactoring:
- `setup_audio()` split into `build_streams()` (pub, per-device) +
  inline param creation (once at startup). `AudioSession` → `AudioStreams`.
- `TonismApp` now owns `Option<AudioStreams>` and can tear down/rebuild
  on device change. Params survive via `FloatParamHandle::build_smoothed()`.
- GUI device picker panel: ComboBox for input/output device, sample rate,
  buffer size. "Apply" button triggers stream rebuild. "Refresh" re-enumerates.
- Config fallback chain: CLI flags > persisted config > system default.
- `find_device()` replaced by `device::resolve_initial_config()`.

Pending: manual verification session.

## Phases ahead

| Phase | What lands                                                                                                                                                                                                                                |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **H** | Cutover: remove the `nih_export_standalone!` runtime use (currently gated on `plugin-export`); update `docs/standards/architecture.md` "pending decisions" section (composition-root location + lock-free GUI↔audio primitive — both now resolved). |

## Verification baseline (last run: 25/05/2026)

- **Default features** — 68 tests pass (60 lib + 5 integration + 3 smoke; 1 doctest ignored). `eframe` + `egui` + `serde` + `confy` in dep graph; `nih_plug` absent.
- **`--features plugin-export`** — compiles clean.
- **`--features debug-assert-no-alloc`** — compiles clean.
- **`--features plugin-export,debug-assert-no-alloc`** — compiles clean.
- **`cargo run`** — eframe window opens with device picker panel; dropdowns populated with system devices; Apply rebuilds streams; sliders, bypass, test signal, xrun counter, latency meter all functional.
- **`cargo run -- --input "BlackHole" --output "BlackHole"`** — CLI flags override persisted config.
- **`cargo run --bin feedback`** — headless stdin-blocking path loads persisted config.

## References

- [ADR-005](../../adr/005-standalone-audio-cpal-direct.md) — pivot decision (partially supersedes ADR-002)
- [ADR-006](../../adr/006-gui-library-after-cpal-direct.md) — GUI library re-evaluation
- [spec.md](spec.md) — full implementation spec (components C1–C10, phases A–H)
