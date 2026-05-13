# Reverse GUI Library Choice — Adopt egui

Owner: Z3U2

Last update: 13/05/2026

**Status:** Accepted — supersedes [ADR-003](003-gui-library.md).

## Context

---

[ADR-003](003-gui-library.md) chose `vizia` (via `vizia/vizia-plug`) over `egui`
on two arguments: (a) vizia ships audio-specific widgets, and (b) egui's
immediate-mode repaint cost was a forward-looking concern for v0.2 plugin
export inside DAW hosts. Both arguments were defensible with the information
available on 07/05/2026.

Building the MVP standalone window against vizia and then re-building it
against egui on `feat/experiment-with-egui` produced new information that
flips the call. This ADR records the reversal so future readers don't have
to re-derive it from commit history.

## New information

---

| # | Finding | Where the original ADR ranked it |
| --- | --- | --- |
| 1 | **Live responsiveness favours egui.** On the same hardware, same `nih_export_standalone!` boot path, same four-control-plus-two-label UI surface, the egui build feels more responsive than the vizia build. The immediate-mode repaint cost flagged in ADR-003 column 2 (`nih_plug_egui`) was speculative; it does not bite the MVP UI surface, and the standalone (non-plugin) path is where the MVP lives. | Predicted egui would lose on responsiveness in plugin context — but standalone is not plugin context. |
| 2 | **Vizia's reactive-binding scaffolding is verbose at this scale.** The xrun-counter polling pattern in the original [editor.rs](../../src/gui/editor.rs) required a `SyncSignal<u64>` + 16 ms `Timer` + `Memo` (14 lines). The egui equivalent is one line: `ui.label(format!("xrun: {}", counter.load(Relaxed)))` inside the per-frame closure, with `ctx.request_repaint_after(16ms)` to keep the loop spinning. Multiply by every future read-only readout (latency display in mvp-02, per-block meters in v0.2) and the cost compounds. | Not weighed — ADR-003 rated both libraries on widget readiness, not on read-only-state-display ergonomics. |
| 3 | **Adapter sustainability for vizia is single-source.** The ADR-003 2026-05-07 update already retracted the "two maintained adapters" advantage — only `vizia/vizia-plug` remains. Egui's adapter (`nih_plug_egui` in BillyDM's fork) is co-located with the chosen runner ([ADR-002](002-standalone-runner.md)) and maintained by the same author who maintains `egui-baseview` underneath it. Fewer moving maintainers in the load-bearing path. | ADR-003 originally scored 🟠🟠 for vizia and 🟢🟢🟢🟢 for egui on this axis; the 2026-05-07 update brought vizia down to 🟠 but did not re-evaluate the recommendation. |

## Decision

---

Adopt **`nih_plug_egui`** from [BillyDM's nih-plug fork](https://github.com/BillyDM/nih-plug)
as Tonism's GUI adapter. Drop `vizia/vizia-plug` and the implicit chain of vizia,
vizia-plug, and `nih_plug_vizia`.

The standalone path stays identical: `nih_export_standalone!(TonismPlugin)`
remains the entry point. Only the contents of `src/gui/editor.rs` change, plus
the GUI dep in [Cargo.toml](../../Cargo.toml). Domain (`src/domain/`) and audio
(`src/audio/`) are untouched, per architecture rule A4 — the swap is exactly
the kind of move A4 is designed to permit.

## Implementation gotchas — record so the next dev doesn't repeat the debug cycle

---

The actual MVP swap on `feat/experiment-with-egui` surfaced three non-obvious
build-and-render issues. All three are recorded here because each one shows
up as "widgets render but text doesn't" — the same symptom for different
underlying causes.

1. **Wrap the update body in `egui::CentralPanel::default().show_inside(ui, |ui| { ... })`.**
   The `nih_plug_egui` update closure receives a root `&mut Ui`, not a `Context`.
   Drawing widgets onto the root `Ui` directly produces visible widgets but
   no rendered text — text never makes the framebuffer. The canonical pattern
   from the fork's [`baseview-adapters/egui-baseview/examples/hello_world.rs`](https://github.com/BillyDM/nih-plug/blob/main/baseview-adapters/egui-baseview/examples/hello_world.rs)
   is `CentralPanel::default().show_inside(ui, |ui| { /* draw here */ })`.

2. **Opt into `egui/default_fonts` explicitly in [`Cargo.toml`](../../Cargo.toml).**
   The fork's workspace declares
   `egui = { version = "0.34.1", default-features = false, features = ["bytemuck"] }`
   — explicitly stripping the bundled UI font to keep plugin binaries small.
   Cargo unifies features additively across the dep graph, so our direct
   `egui` dep must add `features = ["default_fonts"]`. Without it the font
   atlas is empty and text renders as broken glyph fragments (visible
   widgets, scrambled labels).

3. **No `param_button` for `BoolParam`.** Use `ui.checkbox(&mut local, "label")`
   plus a manual `setter.begin_set_parameter / set_parameter / end_set_parameter`
   triplet on `.changed()`. The `FloatParam` case is well-served by
   `nih_plug_egui::widgets::ParamSlider::for_param(&p, setter).ui(ui)`.

4. **No `param_knob` ships either.** ADR-003's "one evening of custom widget
   on top of `param_slider`" tradeoff is unchanged — the cost transfers to egui
   1:1. Defer until v0.2 if the MVP slider widget is acceptable in the meantime.

## Consequences

---

### Positive

- ~14 lines of vizia reactive-binding scaffolding (`SyncSignal` + `Timer` + `Memo`)
  collapses to a one-line atomic read per frame for every read-only readout.
  The mvp-02 latency display and v0.2 per-block meters inherit this simplification.
- Adapter (`nih_plug_egui`), windowing layer (`egui-baseview`), and the chosen
  runner ([ADR-002](002-standalone-runner.md): nih-plug) are now all maintained
  in the same fork by the same author. Fewer single-maintainer hops in the
  load-bearing path.
- egui itself is the healthiest dep on the original ADR-003 page
  (29k stars, sponsored full-time by Rerun). The "Solo-dev sustainability"
  row that scored 🟢🟢🟢🟢 in ADR-003 was always egui's strongest column.

### Negative

- **Visual polish for v0.2 plugin export remains an open question.** Egui's
  default look is harder to brand than vizia or iced. This was ADR-003's
  central argument and it has not been retracted — only deferred. The MVP UI
  is too small a surface to test it. When the v0.2 multi-block chain UI
  lands and we ship a plugin build, re-evaluate. If the look isn't acceptable,
  the choice reopens then.
- **Version-skew tripwire on the direct `egui` dep.** Tonism's
  [`Cargo.toml`](../../Cargo.toml) carries `egui = { version = "0.34.1", … }`
  to opt `default_fonts` back in. The fork's workspace egui version is the
  ground truth; if the fork bumps it, our pin must follow or the build will
  break loudly (different concrete `Visuals` / `CentralPanel` types).
  Loud-failure mode beats silent ABI mismatch, but worth knowing.

### Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| `nih_plug_egui` is single-maintainer (BillyDM). | Same maintainer as the chosen runner per ADR-002 — risk is already accepted at the same tier, not stacked. ISC licensed; forkable. |
| v0.2 plugin-export visual-polish concern from ADR-003 still applies. | Re-evaluate when v0.2 multi-block UI lands. The implementation footprint of an egui→vizia swap is bounded to `src/gui/editor.rs` + Cargo.toml — the same envelope this ADR walked through. |
| Direct `egui` version-pin can drift from the fork. | Loud build failure on mismatch; document the pin alongside the fork patch comment in [`Cargo.toml`](../../Cargo.toml). |

## Follow-ups

---

- [`docs/specs/mvp/stories/mvp-02-latency-readout-in-standalone-implementation-plan.md`](../specs/mvp/stories/mvp-02-latency-readout-in-standalone-implementation-plan.md)
  Section 4.3 was written against vizia idioms (`SyncSignal`, `Timer`, `Memo`,
  `Button::on_press`). Before mvp-02 dev starts, rewrite Section 4.3 to use
  egui's per-frame model (atomic read in the update closure, `ui.button(...).clicked()`,
  per-frame `LatencyDisplay` derivation). The story's acceptance criteria do
  not change — only the GUI shape it describes.
