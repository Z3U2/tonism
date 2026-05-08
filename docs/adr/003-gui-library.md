# GUI Library for nih-plug Application

Owner: Z3U2

Last update: 07/05/2026

## Context

---

[ADR-002](002-standalone-runner.md) committed Tonism to **nih-plug standalone**
as the audio entry point and explicitly deferred the GUI library choice here.
nih-plug ships first-class adapter crates for three Rust GUI libraries; picking
one of them keeps the parameter system, state-binding, and standalone/plugin
boot path on the supported path. Picking outside that set means re-implementing
the adapter ourselves and forfeiting the v0.2 plugin-export reuse promised by
ADR-002.

The MVP UI is small ([spec.md](../specs/mvp/spec.md)): two gain knobs, a bypass
toggle, a test-signal toggle, a latency-ms display, and a live xrun counter.
Knob and meter quality dominate the user perception because they are the only
controls a guitarist sees in-session. The choice also sets the widget vocabulary
the v0.2 effects-chain UI will inherit, so a thin or quirky widget set forces a
costly rewrite later.

## Selection criteria

---

| #   | Criteria                                  | Why this criteria ?                                                                                                                                                                                                                                       |
| --- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Audio-widget readiness                    | Knob, meter, and value-bar widgets are the entire MVP UI surface. A library that ships them tested saves the only widget work the MVP cannot afford to invent.                                                                                            |
| 2   | nih-plug adapter maturity                 | The adapter is the load-bearing seam between GUI thread and audio thread. A buggy or stale adapter forces us to debug someone else's framework instead of shipping the MVP.                                                                               |
| 3   | Time to first rendered knob               | The MVP is one week solo. Hours of boilerplate before the first knob renders eats budget meant for the < 10 ms / zero-xrun stability work in the [acceptance criteria](../specs/mvp/spec.md#acceptance-criteria).                                         |
| 4   | Headroom for v0.2 effects-chain UI        | Per [product architecture](../specs/product-architecture.md#product-layers), v0.2 ships a multi-block signal chain UI (drag/drop reorder, per-block panels). A widget set that can't grow into that costs a GUI rewrite.                                  |
| 5   | Solo-dev sustainability                   | Tonism is a personal OSS tool ([strategic intent](../specs/product-architecture.md#strategic-intent)). A dependency that loses its primary maintainer mid-roadmap is a single-maintainer project's biggest risk.                                          |

## Evaluation

---

> Context for every row:
>
> - The original `robbert-vdh/nih-plug` is no longer maintained
>   ([issue #265](https://github.com/robbert-vdh/nih-plug/issues/265), March 2026).
>   The maintained fork is [`BillyDM/nih-plug`](https://github.com/BillyDM/nih-plug)
>   (mirrored on [Codeberg](https://codeberg.org/BillyDM/nih-plug)). Adapter
>   maturity below is scored against that fork.
> - All three adapters share the same windowing backend (`baseview`) and
>   therefore inherit the **same accessibility gap**: screen-reader support
>   that works in vizia/egui via winit is broken via baseview
>   ([RustAudio/baseview#200](https://github.com/RustAudio/baseview/issues/200)).
>   Not a tie-breaker — it affects all three equally — but worth knowing.

| Solution             | 1. Audio-widget readiness                                                                                                                                                                                                                                                                                                                                                       | 2. nih-plug adapter maturity                                                                                                                                                                                                                                                                                                                                                                                                                          | 3. Time to first rendered knob                                                                                                                                                                                                                                                                                                                                | 4. Headroom for v0.2 effects-chain UI                                                                                                                                                                                                                                                                                                                                                                                       | 5. Solo-dev sustainability                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`nih_plug_vizia`** | 🟢🟢<br>- Ships `param_slider`, `param_button`, `peak_meter`, `generic_ui`, `resize_handle` ([source](https://github.com/robbert-vdh/nih-plug/tree/master/nih_plug_vizia/src/widgets))<br>- **No `param_knob`** in the adapter — production plugins (e.g. [Maerorr's VST3s](https://github.com/Maerorr/maerors-vst3-plugins)) roll a custom `KnobParam` on top of `param_slider` | 🟢🟢🟢<br>- Two maintained adapters: `nih_plug_vizia` inside BillyDM's fork, **plus** [`vizia/vizia-plug`](https://github.com/vizia/vizia-plug) maintained by the vizia org and explicitly tracking newer vizia<br>- vizia 0.4.0 released April 2026 ([source](https://github.com/vizia/vizia))<br>- Tutorial book: [`vizia/vizia-plug-book`](https://github.com/vizia/vizia-plug-book)                                                                | 🟢🟢🟢<br>- Declarative reactive model — `gain_gui_vizia` example shows the boilerplate<br>- Knob gap costs ~1 evening of custom widget on top of `param_slider`<br>- Hours, not days                                                                                                                                                                          | 🟢🟢🟢<br>- Built specifically for audio-plugin UIs — strong theming, ~25 ready-made views<br>- Production VST3s in the wild prove it scales beyond a single-knob page<br>- Best polish story of the three for branded plugin look                                                                                                                                                                                          | 🟠🟠<br>- vizia 2.1k stars, small core team ([source](https://github.com/vizia/vizia))<br>- Pre-1.0 (0.4.0) — API churn possible<br>- Adapter sustainability is good (two parties maintain it), but the underlying framework has the smallest community of the three                                                                                                                                                                                                     |
| **`nih_plug_iced`**  | 🟠<br>- [`iced_audio`](https://github.com/iced-rs/iced_audio) ships Knob, Sliders, Ramp, XY Pad, ModRangeInput on paper<br>- **Last `iced_audio` release: Nov 2020** (v0.5.0); 214 stars; depends on iced 0.8<br>- Ecosystem effectively decayed — widgets exist but won't compile against any current iced                                                                      | 🔴<br>- `nih_plug_iced` pins **iced 0.4** via a custom `iced_baseview` branch ([Cargo.toml source](https://github.com/robbert-vdh/nih-plug/blob/master/nih_plug_iced/Cargo.toml))<br>- Upstream iced is at **0.14.0** (Dec 2025) — **10 major generations behind** ([source](https://github.com/iced-rs/iced))<br>- No public roadmap to bridge the gap; we'd be stuck on a years-old iced for the foreseeable future                                  | 🟠🟠<br>- Elm architecture (`Message` enum + `update` + `view`) is the most ceremony of the three for a small UI<br>- iced 0.4 + ancient `iced_audio` means likely fighting build errors before the first knob renders                                                                                                                                         | 🟢🟢<br>- Elm architecture suits state-rich UIs; modern iced has good drag/drop and layered rendering<br>- But headroom only matters if the adapter catches up — currently we'd build v0.2 on iced 0.4 too                                                                                                                                                                                                                  | 🟢🟢🟢<br>- iced 30.4k stars, sponsored by Cryptowatch/Kraken, Héctor Ramón leads ([source](https://github.com/iced-rs/iced))<br>- Healthy framework upstream<br>- Sustainability of the *adapter* (vs the framework) is the weak link — see column 2                                                                                                                                                                                                                    |
| **`nih_plug_egui`**  | 🟢🟢<br>- egui itself ships no knob or peak meter<br>- Active third-party crates fill the gap: [`egui_knob`](https://crates.io/crates/egui_knob) (v0.3.3, updated 3 months ago), [`egui-audio`](https://github.com/Cannedfood/egui-audio) widget collection<br>- Same widget-effort tier as vizia; the gap is a different widget (meter vs knob)                                | 🟢🟢<br>- The adapter's own README explicitly states: _"Consider using `nih_plug_iced` or `nih_plug_vizia` instead"_ ([source](https://github.com/robbert-vdh/nih-plug/blob/master/nih_plug_egui/README.md)) — no reason given, but it's a maintainer signal that the egui adapter is the least-recommended path<br>- BillyDM maintains [`egui-baseview`](https://github.com/BillyDM/egui-baseview) and the nih-plug fork, so the adapter does work and stay current<br>- Likely reasons for the nudge: immediate-mode repaint cost inside DAW hosts, weaker parameter-binding ergonomics, less polished default look — all weigh more for plugin export (v0.2) than for the MVP standalone<br>- Independently corroborated: a [users.rust-lang.org thread on plugin GUIs](https://users.rust-lang.org/t/need-help-gui-for-a-plugin/111610/2) describes egui's `eframe` as "intended for simple use cases" with advanced cases requiring "a bit more work" — same direction as the README nudge | 🟢🟢🟢🟢<br>- Immediate-mode: simplest mental model in Rust GUI<br>- `egui_knob::Knob::new(...).ui(ui)` is one line<br>- The `gain_gui_egui` reference plugin is the smallest example in the nih-plug repo                                                                                                                                                       | 🟢🟢<br>- Drag-and-drop reorderable lists are well-trodden (`egui_dnd`)<br>- **Weakness:** immediate-mode default look is harder to brand than vizia/iced for a polished plugin UI<br>- Bridgeable but the costliest of the three for visual polish in v0.2; the maintainer's nudge in column 2 is partly about this                                                                                                          | 🟢🟢🟢🟢<br>- egui 29k stars, latest 0.34.2 (May 4, 2026), maintainer Emil Ernerfeldt sponsored full-time by Rerun (which depends on egui) ([source](https://github.com/emilk/egui))<br>- Largest contributor pool of the three<br>- Healthiest dependency on the page                                                                                                                                                                                                   |

## Recommendation

---

Adopt **`nih_plug_vizia`**, consumed via [`vizia/vizia-plug`](https://github.com/vizia/vizia-plug)
so the adapter tracks current vizia (the in-tree adapter lags). This aligns with
the framework maintainer's explicit steering away from egui (the
[`nih_plug_egui` README](https://github.com/robbert-vdh/nih-plug/blob/master/nih_plug_egui/README.md)
recommends iced or vizia instead) and with independent community signal that
egui suits simple UIs and scales less well into branded plugin look-and-feel
needed for v0.2 plugin export. iced is excluded as effectively non-viable —
`nih_plug_iced` is pinned to iced 0.4 while upstream is at 0.14, and
`iced_audio` has had no release since 2020.

The accepted tradeoffs are: (1) one evening of custom widget work to build a
knob on top of `param_slider` (no `param_knob` ships); (2) vizia has the
smallest community of the three candidates, mitigated by **two** maintained
adapters (`vizia/vizia-plug` from the vizia org, `nih_plug_vizia` in BillyDM's
fork) and accepted at the same risk tier as ADR-002.

**Follow-up needed:** [ADR-002](002-standalone-runner.md) should be amended to
point at [`BillyDM/nih-plug`](https://github.com/BillyDM/nih-plug) as the
maintained fork — the original is no longer maintained
([issue #265](https://github.com/robbert-vdh/nih-plug/issues/265)).

## Update — 2026-05-07

The macOS/Windows compile bug discovered during MVP scaffolding (Phase 2 of
the project plan) forced
verification of this ADR's "two maintained adapters" claim. Live inspection of
[`BillyDM/nih-plug/tree/master/crates`](https://github.com/BillyDM/nih-plug/tree/master/crates)
shows the fork ships `nih_plug_core`, `nih_plug_derive`, `nih_plug_egui`, and
`nih_plug_iced` — but **not** `nih_plug_vizia`. Vizia was dropped from the fork
during the recent `nih_plug_core` split.

Only [`vizia/vizia-plug`](https://github.com/vizia/vizia-plug) remains as a
maintained vizia-based nih-plug adapter. The crate is named `vizia_plug`
(snake_case), not `nih_plug_vizia`. Tonism's [`Cargo.toml`](../../Cargo.toml)
uses `vizia_plug = { git = "https://github.com/vizia/vizia-plug" }` plus a
[`[patch]`](../../Cargo.toml) routing `nih_plug` through a personal fork
([`Z3U2/nih-plug`](https://github.com/Z3U2/nih-plug) branch
`fix-macos-windows-vst3-import`) until the upstream macOS/Windows fix lands.

**Effect on the matrix:** the "Solo-dev sustainability" cell for `nih_plug_vizia`
above (🟠🟠) was predicated on two maintained adapters. With BillyDM's vizia
adapter gone, the realistic score is **🟠** (single maintained adapter from the
vizia org).

**Effect on the recommendation:** **stands.** The other two candidates are still
worse:

- `nih_plug_iced` is pinned to iced 0.4 while upstream iced is at 0.14, and
  `iced_audio` has had no release since 2020 — non-viable.
- `nih_plug_egui` carries the framework maintainer's explicit nudge against it
  for plugin UIs and the immediate-mode-in-DAW-host repaint cost flagged in
  the matrix.

`vizia/vizia-plug` is actively maintained (last push 2026-04-30). The
recommendation to adopt a vizia-based adapter therefore holds; the **source**
was always `vizia/vizia-plug` (per the original Recommendation line above) —
this update merely retires the now-unavailable BillyDM in-tree backup.
