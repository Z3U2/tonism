# Standalone Audio Path — Direct cpal

Owner: Z3U2

Last update: 20/05/2026

**Status:** Accepted — partially supersedes [ADR-002](002-standalone-runner.md).
ADR-002's nih-plug recommendation remains the prevailing call for the future
VST3 / CLAP export path; this ADR replaces it only for the **standalone**
boot path that the MVP ships on.

## Context

---

The MVP standalone, built on `nih_export_standalone!(TonismPlugin)` per
[ADR-002](002-standalone-runner.md), produces audible **crackling / choppy
output** in every configuration we have been able to construct:

| Axis | Values tried |
| --- | --- |
| OS / backend | macOS / CoreAudio · Windows / WASAPI |
| Sample rate | 44.1 kHz · 48 kHz · 96 kHz |
| Buffer size | nih-plug standalone CLI sweeps (small → large) |
| Physical devices | MacBook built-in · External USB headphones · External USB mic · Focusrite Solo |
| Code under test | Tonism's `Plugin::process` · upstream **nih-plug's own hello-world examples** |

The artifact reproduces on the upstream examples — that is, with **zero
Tonism code in the signal path** — which rules out a bug introduced by our
domain layer, our parameter wiring, or our GUI integration. The same
machines run the upstream **`cpal` feedback example** cleanly (mic →
headphones, no DSP), which rules out cpal itself, the OS audio stack, the
devices, and the user's environment.

The bug is therefore somewhere in the nih-plug standalone wrapper (its cpal
→ `Plugin::process` glue: stream configuration negotiation, buffer
shuttling, channel layout coercion, or the standalone event loop). Whether
the root cause is upstream code, our `AUDIO_IO_LAYOUTS`, or an interaction
between the two is unknown; what is known is that a week of debugging on
multiple machines has not narrowed it.

We have not yet tried the VST3 / CLAP build path. Doing so to "prove the
DSP is fine" before pivoting would burn budget on a target the MVP does
not ship and would not change the standalone problem.

## New information

---

| # | Finding | Where ADR-002 ranked it |
| --- | --- | --- |
| 1 | **`cpal` works; the wrapper around it does not.** ADR-002 column 1 ("Real-time audio safety guarantees") rated nih-plug 🟢🟢🟢🟢 on the strength of `HARD_REALTIME_ONLY` + `SAMPLE_ACCURATE_AUTOMATION` constants and the `rtrb`-backed parameter ring. Those guarantees are real in the type system but did not prevent audible xruns / choppy output in practice on our two MVP target OSes. The same machines run the cpal feedback example clean, so the operational guarantee the column was bought on is not being delivered on the standalone path. | Predicted 🟢🟢🟢🟢. Empirically not delivered for the standalone target. |
| 2 | **"Time to first audible signal" was paid for, but is being paid back with interest in debugging.** ADR-002 column 2 valued `nih_export_standalone!` as hours-not-days to a working binary. That was accurate for code-to-compile. The end-to-end "clean guitar through headphones" target is still not met after a week of operational debugging, against an estimated 1–2 days of glue to wire cpal directly per the `feedback.rs` example shape. | Predicted 🟢🟢🟢🟢. Net of debug cost, the advantage has inverted. |
| 3 | **The hexagonal seam (A4) is exactly what makes this swap cheap.** The domain layer (`src/domain/`) has zero imports from `nih_plug`. `Plugin::process` in `src/audio/plugin.rs` is the only place that calls into the chain — replacing it with a cpal stream callback that calls the same `Process::process(&mut [f32])` on the same `Gain` block is a contained change. The cost we are accepting is rebuilding the parameter ring and the GUI host outside nih-plug, not rewriting any DSP. | Not weighed at decision time — A4 was unused because nothing had moved. |
| 4 | **VST3 / CLAP reuse is preserved, not destroyed.** Nothing in this ADR removes the `TonismPlugin: Plugin` impl. It stays in `src/audio/plugin.rs` as a parallel target for the v0.2+ plugin-export path that ADR-001 / ADR-002 always planned for. The standalone binary stops going through it; that is the only change to ADR-002's reuse argument. | ADR-002 column 4 stays 🟢🟢🟢🟢 for the future plugin path. |

## Decision

---

The standalone binary stops booting through `nih_export_standalone!` and
boots through a **direct cpal entry point** owned by Tonism. The cpal
`feedback.rs` example is the known-good starting shape we incrementally
extend toward MVP feature parity.

The `Plugin` impl in `src/audio/plugin.rs` is **retained as dormant code**
for the future VST3 / CLAP build target. It is excluded from the default
build's runtime path; whether it stays compiling continuously or is gated
behind a `--features plugin-export` flag is a build-system call deferred to
the implementation spec.

The GUI library choice from [ADR-004](004-gui-library-egui.md) (egui) is
unchanged. The `nih_plug_egui` *adapter* is replaced with a direct egui
host (egui-winit + a backend renderer, exact crate selection deferred to
the spec). The domain layer is untouched, per architecture rule A4.

The implementation plan lives at
[`docs/specs/cpal-direct-standalone/spec.md`](../specs/cpal-direct-standalone/spec.md).

## Implementation gotchas — record so the next dev doesn't re-derive them

---

These are the load-bearing constraints the spec must respect; they are
recorded here because they are decision-level, not implementation detail.

1. **Anchor on `feedback.rs`, not on a clean-sheet design.** The whole
   point of this pivot is that the cpal feedback example works on our
   hardware. The first walking skeleton must be that example with the
   Tonism name on it — no domain, no GUI, no parameter system. Each
   subsequent phase adds exactly one component and re-verifies clean
   audio. If a phase introduces choppiness, the suspect surface is one
   commit deep, not a week of nih-plug internals.

2. **The new parameter system is the architectural debt ADR-002 explicitly
   avoided.** ADR-002's recommendation paragraph called out that "every
   alternative requires us to invent the GUI ↔ audio parameter
   primitive — exactly the `TBD` the architecture standards explicitly
   defer." We are now taking on that debt. The architecture-standard rule
   F4 ("GUI → audio one-way through a lock-free channel") plus rule A2
   ("no alloc / lock / syscall on the per-buffer path") are the
   non-negotiable constraints the new primitive must satisfy. The spec
   names the primitive; this ADR only records that it is now in scope.

3. **Sample-accurate smoothing must be re-implemented.** nih-plug's
   `Smoother` advanced once per frame inside `Plugin::process`; the new
   per-param smoother must offer the same per-frame `.next()` semantics
   so the gain trims keep the click-free behaviour the MVP already has.
   This is pure-Rust, lock-free, no I/O — it can live in the domain.

4. **Persistence of param state across stream restarts is a new
   requirement.** nih-plug held param state outside `Plugin::initialize`;
   the new architecture must keep params alive across device / sample-rate
   reconfiguration (cpal stream tear-down + rebuild) so the user does not
   lose their settings on every device change. This is owned by the new
   parameter system, not the audio backend.

5. **Do not chase a nih-plug root-cause as a precondition.** The
   diagnostic budget for "what specifically is nih-plug standalone doing
   wrong" is now zero. If a root cause surfaces incidentally during the
   pivot (e.g. by re-reading wrapper source while writing our own), record
   it in the spec's References section and move on. The decision does not
   depend on the answer.

## Consequences

---

### Positive

- **The known-good baseline (cpal feedback) is the new starting point.**
  Audio quality regressions during the rebuild are bounded to the single
  phase that introduced them, not buried in upstream wrapper code.
- **We own every line of the audio entry path.** Future debugging is
  reading our own source, not bisecting an external standalone wrapper
  across releases.
- **Architecture rule A4 stops being theoretical.** The domain layer
  proves it can live behind a swappable shell when the shell actually gets
  swapped. The same property buys back the v0.2+ plugin-export path
  cheaply.
- **The GUI ↔ audio bridge stops being a transitive dep of nih-plug.**
  `XrunCounter`, `LatencyHandle`, and the `log_bridge` already exist as
  first-class lock-free primitives in `src/audio/`; they keep working
  unchanged.

### Negative

- **We pay the architectural debt ADR-002 deferred.** Parameter system,
  parameter smoothing, parameter persistence, and the GUI host are now
  Tonism's to write and maintain. The MVP spec budget for these is
  non-trivial.
- **No automatic VST3 / CLAP build from `Cargo.toml`-flag flip anymore.**
  When v0.2+ plugin export lands, we either resurrect the dormant `Plugin`
  impl through a feature flag (then it had better still compile) or
  re-derive it. ADR-002's "free reuse" property is downgraded to "parallel
  target maintained at some cost."
- **Time-to-MVP-shipping slips.** The cpal-direct rebuild is a week-scale
  task on top of the week already spent. This is a real budget hit; it is
  accepted because the alternative (continued nih-plug-standalone
  debugging with no convergence signal) has unbounded slip risk.

### Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| The choppy-audio root cause is in cpal itself or in our OS configuration, and the rebuild reproduces it. | The cpal feedback example is the falsifier: if the walking-skeleton phase A is choppy, the root cause is below nih-plug and this ADR is wrong. Phase A must run for a full 5-minute clean-audio session before any subsequent phase ships. |
| The DIY parameter system has subtle A2 violations under load (alloc / lock / syscall not visible at code-review time). | The existing `debug-assert-no-alloc` build feature (mapped to `nih_plug/assert_process_allocs`) needs an equivalent for the new path. Spec must name the replacement primitive (e.g. `assert_no_alloc` crate or hand-written guard) before phase C. |
| The dormant `Plugin` impl rots and is uncompilable when v0.2 plugin export starts. | Either gate it behind a `plugin-export` cargo feature that CI compiles on every push, or accept that it will need re-derivation — recorded explicitly when v0.2 planning starts. Choice is a spec-level decision, not an ADR decision. |
| Window / GUI integration (`nih_plug_egui` → direct egui host) introduces its own crackling via a frame-rate or vsync interaction with the audio thread. | Phase D of the spec stands the window up against a running audio stream with **no** GUI ↔ audio coupling, to isolate "does adding a window break audio" from "does GUI traffic break audio." |
| Single-maintainer exposure was the headline cost of ADR-002; this ADR adds two new single-org dependencies (`cpal`, the chosen egui-winit backend) to the load-bearing path. | `cpal` is RustAudio org, ~3.7k stars, already a transitive dep today. `egui` / `egui-winit` are the healthiest-maintained crates in the original ADR-003 page. The net dependency-health risk is lower than the nih-plug standalone path it replaces. |

## Follow-ups

---

- [`docs/specs/cpal-direct-standalone/spec.md`](../specs/cpal-direct-standalone/spec.md)
  — the implementation plan: components, phases, exit criteria.
- ADR-002 stays in force for the v0.2+ VST3 / CLAP export decision. When
  that planning starts, re-read ADR-002 columns 1, 2, 4 against current
  conditions — they may need their own update by then.
- The `feat/mvp-02-latency-readout` PR (#4) is unaffected by this ADR:
  it merges as-is, on the current nih-plug standalone path. The pivot
  starts from main after that merge.
