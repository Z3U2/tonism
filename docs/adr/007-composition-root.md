# Composition Root — `src/cpal_direct.rs`

Owner: Z3U2

Last update: 2026-06-04

**Status:** Accepted — resolves the "Composition-root location" TBD in
[Architecture Standards](../standards/architecture.md) rule A5.

## Context

---

Architecture rule A5 ("One composition root. DI wiring lives in a single
place at startup.") was recorded as TBD because the prevailing standalone
runner at the time was `nih_export_standalone!(TonismPlugin)` from
[ADR-002](002-standalone-runner.md). With that macro, the nih-plug
framework owned the composition: stream opening, GUI hosting, parameter
wiring, and the top-level loop were all internal to the wrapper. Tonism
had no single file it could point to as the place where everything was
assembled.

[ADR-005](005-standalone-audio-cpal-direct.md) pivoted the standalone
binary away from `nih_export_standalone!` and toward a direct cpal entry
point owned by Tonism. The cpal-direct standalone path was built out
across Phases A–H of the [implementation spec](../specs/cpal-direct-standalone/spec.md).
Spec component C8 explicitly calls out the composition root as one of the
surfaces the new path must supply (see spec.md § C8).

## Decision

---

The single composition root for the Tonism standalone binary is
**`src/cpal_direct.rs`**, wired from **`src/main.rs`**.

`src/cpal_direct.rs` exposes two public entry points:

- `run_gui()` — the default path invoked by `src/main.rs`. Loads
  persisted config, resolves devices, constructs `TonismParams`, calls
  `build_streams()`, then hands ownership of streams and GUI handles into
  `eframe::run_native` (the GUI host, which runs on the main thread as
  required by macOS/winit).
- `run()` — the headless path used by `src/bin/feedback.rs`. Same device
  resolution and stream construction; blocks on stdin instead of opening
  a window.

`build_streams()` is the DI wiring site: it accepts device handles and
param handles, constructs all audio-side state (ring buffer, smoothed
param readers, domain blocks, latency meter), and returns an
`AudioStreams` struct whose drop stops the cpal streams.

**Drop / teardown order** (per spec C8 guarantee):

1. GUI first — `eframe::run_native` returns when the window closes,
   releasing all GUI-side param write handles. No more param writes can
   race with teardown.
2. Audio second — `AudioStreams` is dropped after `run_gui()` returns,
   stopping the cpal input and output streams. The callback cannot fire
   after this point.
3. Telemetry / log last — `XrunCounter`, `LatencyHandle`, and
   `log_bridge` are `Arc`-backed; they outlive both of the above and are
   freed when the last Arc clone drops, which happens after the audio
   thread has exited.

As of the Phase H cutover, `nih_export_standalone!` is no longer on the
default runtime path: `main.rs`'s default `run()` calls
`cpal_direct::run_gui()`, and the macro is invoked only from the
`#[cfg(feature = "plugin-export")]` branch. The dormant
`TonismPlugin: Plugin` impl in `src/audio/plugin.rs` is retained behind
that `plugin-export` cargo feature for the future v0.2+ VST3/CLAP build
target (per ADR-005 and spec C10).

## Consequences

---

### Positive

- Rule A5 is now a concrete, auditable guarantee: any reviewer can verify
  that `src/cpal_direct.rs` is the only file that assembles the full
  component graph.
- Teardown order is deterministic and documented, eliminating a class of
  use-after-free and data-race scenarios between GUI writes and audio reads
  during shutdown.
- Future composition changes (e.g. adding a headless test mode, an HTTP
  control API, or a REPL) have a clear insertion point without touching
  domain or GUI code.

### Negative

- `src/cpal_direct.rs` grows in scope over time as new components are
  wired in. Keeping the wiring readable requires discipline to delegate
  construction into typed builder functions and avoid letting the
  composition root become a god-file.

## Follow-ups

---

- [ADR-005](005-standalone-audio-cpal-direct.md) — the cpal-direct pivot
  this ADR finalises the composition root for.
- [ADR-008](008-lockfree-gui-audio-param.md) — the lock-free param
  primitive wired inside the composition root.
- [`docs/specs/cpal-direct-standalone/spec.md`](../specs/cpal-direct-standalone/spec.md)
  § C8 — the spec contract this ADR implements.
