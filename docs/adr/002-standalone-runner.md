# Standalone Runner & Audio Entry Point

Owner: Z3U2

Last update: 07/05/2026

## Context

---

Tonism is a real-time guitar processor. The MVP (week 1) is standalone-only —
guitar in, processed audio out, headphones, < 10 ms round-trip latency, zero
xruns over a 5-minute session, crash-free 30-minute stress with parameter
changes ([spec.md](../specs/mvp/spec.md)). Per [ADR-001](001-language-choice.md),
the product will later also ship as VST3 and CLAP plugins.

Today the codebase has no audio entry point, no window, and no GUI loop. Before
any DSP work can start, we must decide how the standalone binary boots: who
owns the audio callback, who opens the application window, and who hosts the
GUI. The decision sets the parameter-system contract and determines how much
of the MVP code can be reused when plugin export ships in v0.2+.

The trigger is the MVP [dependencies.md](../specs/mvp/dependencies.md) flagging
this as the top blocking decision before week-1 development can start.

**Constraint:** the runner must build and run on macOS (CoreAudio), Linux
(JACK/PipeWire), and Windows (WASAPI/ASIO). All three OSes must work for the
MVP — guitar in, audio out, GUI window — so cross-platform support is a
prerequisite, not a tie-breaker.

**Note on upstream:** The original `robbert-vdh/nih-plug` repository is no
longer maintained as of March 2026 ([issue #265](https://github.com/robbert-vdh/nih-plug/issues/265)).
The maintained fork is [`BillyDM/nih-plug`](https://github.com/BillyDM/nih-plug)
(also mirrored on [Codeberg](https://codeberg.org/BillyDM/nih-plug)). All
references in this ADR to the nih-plug source point at the BillyDM fork.
[ADR-003](003-gui-library.md) corroborates this finding.

## Selection criteria

---

| #   | Criteria                          | Why this criteria ?                                                                                                                                                                                       |
| --- | --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Real-time audio safety guarantees | MVP success bars are operational ([< 10 ms, zero xruns over 5 min](../specs/mvp/spec.md#acceptance-criteria)). A runner that already enforces no-alloc and lock-free parameter passing protects the bars. |
| 2   | Time to first audible signal      | The MVP is a one-week solo build. Days of glue code before any guitar passes through the path eats the budget meant for stability work.                                                                   |
| 3   | GUI host integration              | The MVP needs a window with knobs, latency display, and an xrun counter visible in-session. A runner that bundles a window + GUI adapter saves choices the MVP can't afford to re-derive.                 |
| 4   | Reusability for plugin export     | ADR-001 keeps VST3/CLAP on the roadmap (post-Q1). A runner whose code can be reused for the plugin build avoids re-engineering the audio entry point later.                                               |
| 5   | Solo-dev sustainability           | Tonism is a personal OSS tool ([strategic intent](../specs/product-architecture.md#strategic-intent)). Single-maintainer dependencies are tolerable if forkable, but maintenance load still matters.      |

## Evaluation

---

| Solution                                     | 1. Real-time audio safety                                                                                                                                                                                                                                                                                                                                                            | 2. Time to first audible signal                                                                                                                                                                                                                                       | 3. GUI host integration                                                                                                                                                                                                                                                       | 4. Reusability for plugin export                                                                                                                                                                                                                              | 5. Solo-dev sustainability                                                                                                                                                                                                                                                                |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **nih-plug standalone** (GUI adapter TBD)    | 🟢🟢🟢🟢<br>- `Plugin::HARD_REALTIME_ONLY` + `SAMPLE_ACCURATE_AUTOMATION` constants enforce RT discipline ([source](https://github.com/BillyDM/nih-plug/blob/master/src/plugin/mod.rs))<br>- Parameter system uses `rtrb` lock-free SPSC ring internally — no DIY channel<br>- `Buffer` iterators designed for per-buffer, no-alloc processing                                  | 🟢🟢🟢🟢<br>- `nih_export_standalone!(MyPlugin)` macro is the entire entry point ([source](https://github.com/BillyDM/nih-plug/blob/master/src/wrapper/standalone/mod.rs))<br>- Standalone CLI for sample rate / buffer size / device selection out of the box          | 🟢🟢🟢🟢<br>- First-class adapter framework for `egui`, `iced`, and `vizia` — choice deferred to a follow-up ADR ([source](https://github.com/BillyDM/nih-plug))<br>- Window opening / sizing / parameter binding wired by the framework regardless of GUI lib | 🟢🟢🟢🟢<br>- Same `Plugin` impl + same `main.rs` exports VST3 (`nih_export_vst3!`) and CLAP (`nih_export_clap!`)<br>- Plugin path in v0.2+ is a `Cargo.toml` change, not a rewrite                                                                            | 🟠🟠<br>- Now stewarded by BillyDM ([BillyDM/nih-plug](https://github.com/BillyDM/nih-plug)), ISC-licensed<br>- ~4.5k stars, used in production CLAP/VST3 plugins ([listed in CLAP DB](https://clapdb.tech/software/58/))<br>- BillyDM's fork is the active maintained tree (original unmaintained per [issue #265](https://github.com/robbert-vdh/nih-plug/issues/265))  |
| **Raw `cpal` + `winit` + `egui`**            | 🟠🟠<br>- `cpal` raises the audio callback to RT priority ([source](https://github.com/RustAudio/cpal))<br>- No parameter-passing primitive — author writes the GUI ↔ audio channel and has to enforce A2 (no alloc / lock / syscall) by hand<br>- Linux needs `rtkit` / `limits.conf` setup that the dev must document                                                              | 🟠🟠<br>- No canonical cpal + winit + egui audio-app example in the search results — integration is DIY<br>- Author wires duplex stream, winit event loop, egui-winit texture upload, GUI ↔ audio queue, device enumeration UI<br>- "Days, not hours" of glue for v0.1 | 🟠<br>- `egui-winit` is mature and cross-platform but ships zero audio-specific widgets — knobs/meters are hand-built<br>- No window-bring-up helper for an audio app; all event-loop wiring is the author's                                                                  | 🔴<br>- None. Plugin export later means picking nih-plug or another framework and re-implementing the audio entry point + parameter binding from scratch<br>- DSP core can transfer; the entry point and GUI binding cannot                                   | 🟢🟢🟢🟢<br>- `cpal` (~3.7k stars), `winit` (de facto Rust window crate), `egui` (~28k stars, very active) — all multi-maintainer<br>- The healthiest dependency surface of the three options                                                                                            |
| **`baseview` + `cpal` + `vizia`**            | 🟢🟢<br>- `cpal` callback is RT-priority<br>- `vizia` is plugin-aware (used inside `nih_plug_vizia`) but does not ship a parameter-passing primitive — author writes the GUI ↔ audio channel<br>- Better positioned than option 2 because `baseview` was built for audio-plugin UI threading model                                                                                   | 🟢🟢<br>- `baseview` + `vizia` window pattern is documented; `vizia` ships knob/meter widgets<br>- `cpal` duplex stream + GUI ↔ audio queue still on the author<br>- Faster than option 2 (no manual GUI wiring), slower than option 1 (no ready entry point)          | 🟢🟢🟢<br>- `vizia` ships audio-specific widgets (knob, meter) — same widget set you'd use under `nih_plug_vizia`<br>- `baseview` handles cross-platform window opening for plugin-style UIs ([source](https://github.com/RustAudio/baseview))                                | 🟠🟠<br>- Partial. The `vizia` GUI code transfers to `nih_plug_vizia` later, but the `cpal` audio entry point and any author-built parameter channel must be replaced<br>- Better than option 2 (GUI survives) but no automatic VST3/CLAP build path          | 🟠🟠<br>- `baseview` actively maintained (last update [Jan 12, 2026](https://github.com/RustAudio/baseview)), RustAudio org<br>- `vizia` smaller community than `egui`; `nih_plug_vizia` is the main consumer driving its plugin API surface<br>- Two single-org dependencies stacked     |

## Recommendation

---

Adopt **nih-plug standalone** as the audio entry point and parameter host for
the MVP. It dominates the four top criteria: `HARD_REALTIME_ONLY` and an
`rtrb`-backed parameter system enforce A2 at the type level; `nih_export_standalone!`
is the entire boot path (hours, not days, to first guitar through headphones);
the framework provides first-class adapter traits for any of `egui`, `iced`, or
`vizia`; and the same `Plugin` impl becomes VST3 + CLAP in v0.2+ with a
`Cargo.toml` flag flip rather than a rewrite. Every alternative requires us to
invent the GUI ↔ audio parameter primitive — exactly the `TBD` the architecture
standards explicitly defer.

The accepted tradeoff is single-maintainer exposure. It is mitigated by ISC
licensing, active stewardship under [BillyDM's fork](https://github.com/BillyDM/nih-plug)
(now the maintained home following the original repo's retirement per
[issue #265](https://github.com/robbert-vdh/nih-plug/issues/265)), and the fact
that ADR-001 already accepted this risk.

The choice of GUI library (`egui` vs `iced` vs `vizia`) is **deferred to a
follow-up ADR**, since all three are first-class nih-plug adapters and the
runner decision is independent of widget set.
