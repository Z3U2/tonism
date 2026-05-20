# Direct-cpal Standalone — Implementation Spec

Owner: Z3U2

Last update: 20/05/2026

**Status:** Drafted alongside [ADR-005](../../adr/005-standalone-audio-cpal-direct.md).
This spec covers the **what** (abstract components, phase exits, risks);
the **how** (crate names beyond cpal/egui, struct shapes, channel
capacities) is decided at implementation time, not here.

## Context

---

Per [ADR-005](../../adr/005-standalone-audio-cpal-direct.md), the standalone
binary's audio entry point moves from `nih_export_standalone!` to a direct
cpal stream owned by Tonism. The cpal `feedback.rs` example is the known-
good shape we extend from; every phase below ends with a self-contained
verification that the standalone binary still produces clean audio.

The domain layer (`src/domain/`) is untouched. The GUI library is
unchanged (egui per [ADR-004](../../adr/004-gui-library-egui.md)). The
only things being replaced are the **audio entry point**, the **parameter
system**, the **GUI host** (`nih_plug_egui` → a direct egui-winit
adapter), and the **composition root** that wires them together.

## Goals

---

| ID | Goal |
| --- | --- |
| G1 | Standalone binary produces clean audio (5-minute session, zero xruns) on macOS / CoreAudio and Windows / WASAPI, with the user's MVP target devices. |
| G2 | Domain layer (`src/domain/`) compiles and runs unchanged. The pivot proves architecture rule A4 in practice. |
| G3 | Existing MVP feature surface is preserved: input/output gain trims with sample-accurate smoothing, bypass, test-signal toggle, latency meter (mvp-02), xrun counter. |
| G4 | The dormant `Plugin` impl remains a real future target — either compiles continuously under a feature flag, or has a clearly named re-derivation cost when v0.2 plugin export starts. |
| G5 | The MVP success bar from [`docs/specs/mvp/spec.md`](../mvp/spec.md) is reachable end-to-end: < 10 ms round-trip, zero xruns over 5 min, crash-free 30-min stress with parameter changes. |

## Non-goals

---

- VST3 / CLAP plugin export (deferred to v0.2+; ADR-002 stays the relevant
  decision for that path).
- ASIO support on Windows (covered by the separate
  [`docs/specs/windows-asio-support/spec.md`](../windows-asio-support/spec.md);
  the cpal-direct architecture is what makes that ASIO path possible at
  all, but adopting it is its own decision).
- Multi-block signal chain UI (v0.2). The MVP UI surface remains the
  current four-control-plus-two-label shape.
- Preset / project persistence. Param values survive **stream restart**
  inside one session (a hard requirement of the new architecture); they do
  not need to survive across process restarts in the MVP.
- Diagnosing the upstream nih-plug standalone choppiness. Per ADR-005, the
  budget for root-causing the wrapper is zero.

## Abstract components

---

These are the named architectural surfaces the new standalone path
requires. Each entry states **purpose**, **inputs / outputs at the
boundary**, and **guarantees it must provide**. Concrete crate or type
choices are made during implementation; this list is the contract.

### C1 — Audio Backend

**Purpose:** open a duplex audio stream against a chosen input + output
device, drive a per-block callback at realtime priority, signal stream-
level events (xrun, device-disconnect, sample-rate change) back to the
composition root without blocking the callback.

**Boundary:**
- *In:* a chosen `(input device, output device, sample rate, buffer
  size)` quadruple from C7.
- *In:* a callback the audio thread invokes per block.
- *Out:* lifecycle events (started, stopped, errored, xrun observed)
  delivered out-of-thread.

**Guarantees:**
- The callback runs on a realtime-priority thread, exactly per cpal's
  contract.
- No allocation, locking, or syscall is introduced by C1 inside the
  callback. (The callback's *content* is C2's problem; C1 must not add
  anything.)
- Stream open / close are idempotent and tearable from any thread that is
  not the audio thread.

### C2 — Audio Process Function

**Purpose:** the body of the cpal callback. Reads parameter snapshots from
C3, runs the domain chain (`Process::process` on each block), updates the
latency meter (C4-existing) and xrun counter (C5-existing).

**Boundary:**
- *In:* one input slice + one output slice per call (cpal hands these in
  as interleaved or per-channel; channel layout adaptation lives here).
- *In:* read-only handles to C3 (params) and write-only handles to C5
  (xrun, log bridge, telemetry).
- *Out:* nothing returned; side-effect is writing the output slice.

**Guarantees:**
- A2-clean: no alloc, no lock, no syscall.
- Channel-layout adaptation (mono → stereo split, etc.) is explicit and
  testable in isolation, not buried in the callback shape.
- Bypass path is in-place identity (input → output), matching today's
  `Plugin::process` shortcut.

### C3 — Parameter System

**Purpose:** the single source of truth for user-controllable parameters.
Replaces `nih_plug::params`. Provides atomic, lock-free read on the audio
side and a write API on the GUI side, with per-frame smoothing for
parameters that need it.

**Boundary:**
- *In (write side, GUI thread):* `set(param_id, value)` — non-blocking,
  no allocation per call once steady-state.
- *Out (read side, audio thread):* per-frame `.next()` that returns the
  smoothed value at sample-accuracy.
- *Out (read side, GUI thread):* `.value()` for displaying the current
  target back in the UI.

**Guarantees:**
- F4-compliant: GUI → audio is one-way through a lock-free primitive; the
  audio thread never writes back.
- F1-compliant: one authoritative copy per parameter; the smoother is
  derived state, not a parallel mutable copy.
- A2-clean on the read side: `.next()` does no alloc, no lock, no syscall.
- Survives stream restart: when C1 tears the stream down on device /
  sample-rate change, parameter values are preserved.
- Per-parameter metadata (name, range, unit, default, smoothing time
  constant) is declared once at construction; the GUI reads it via the
  same handle.

### C4 — Parameter Smoothing Primitive

**Purpose:** the per-frame `.next()` engine that turns a target value into
a click-free signal at the audio rate. Pure Rust, lock-free, no I/O.

**Boundary:**
- *In:* a `set_target(value)` from C3 on the write side.
- *Out:* a `.next() -> f32` on the read side, advancing one sample per
  call.

**Guarantees:**
- A1-eligible: contains no imports from audio I/O, GUI, plugin host, or
  filesystem crates — lives in `src/domain/` or a domain-adjacent module.
- A2-clean on `.next()`.
- Sample-accurate convergence to the target within the declared smoothing
  time constant; behaviour at sample-rate changes is defined (typically:
  reconstruct the smoother under the new SR).

### C5 — GUI ↔ Audio Telemetry (existing — keeps shape)

**Purpose:** the audio → GUI direction. xrun counter, latency capture
buffer, log bridge. Today this is `src/audio/xrun.rs`,
`src/audio/latency.rs`, `src/audio/log_bridge.rs`.

**Boundary, guarantees:** unchanged from today. The existing primitives
are already lock-free, A2-clean on the write side, and consumed via
atomic reads on the GUI side. The pivot inherits them as-is; what changes
is that they are wired into C1's callback rather than into
`Plugin::process`.

### C6 — GUI Host

**Purpose:** stand up a native window with an egui context, run the
per-frame update closure at ~60 Hz, render. Replaces `nih_plug_egui`'s
window-opening + event-pump role.

**Boundary:**
- *In:* shared handles to C3 (parameter read/write), C5 (telemetry read).
- *In:* the same egui update closure the existing `editor.rs` defines.
- *Out:* close-event signal back to the composition root.

**Guarantees:**
- Runs on its own thread (or the main thread per OS convention); never
  blocks the audio callback.
- Repaint cadence is decoupled from audio block rate.
- Window close → orderly tear-down of C1, C6, and the composition root.

### C7 — Device & Stream Configuration

**Purpose:** enumerate cpal hosts and devices, resolve a user-selected
configuration (CLI flags + persisted last-used + GUI picker), negotiate
sample rate / buffer size against device capabilities, hand the resolved
quadruple to C1.

**Boundary:**
- *In:* CLI argv, optional persisted config file, optional GUI selection
  event.
- *Out:* a resolved `(input device, output device, sample rate, buffer
  size)` quadruple, or a structured error explaining why no quadruple
  could be resolved.

**Guarantees:**
- All device enumeration happens off the audio thread.
- Mono input + stereo output (the MVP's primary shape, per `AUDIO_IO_LAYOUTS[0]`
  in today's plugin) is the default and works without any flag.
- Failure modes are user-actionable (named devices not found → list what
  *was* found; SR negotiation failed → list what *was* supported).

### C8 — Composition Root

**Purpose:** the one place where everything is wired. Owns the lifetime
of C1, C3, C6; constructs the bridges between them; runs the top-level
loop until C6 signals close; tears down in the right order.

**Boundary:** `main.rs`. Takes no inputs except CLI argv and the
filesystem.

**Guarantees:**
- Single composition root (architecture rule A5 — previously TBD because
  the plugin framework owned it; now ours).
- Drop order is correct: GUI first (so it stops requesting param writes),
  audio second (so the callback stops reading), telemetry/log threads
  last (so their consumers have exited).

### C9 — A2 Enforcement (existing — must follow the pivot)

**Purpose:** the `debug-assert-no-alloc` feature today maps to
`nih_plug/assert_process_allocs`. On the new path, the equivalent
must wrap C2's callback so that allocations on the audio thread still
panic loudly in dev builds.

**Boundary:** a build feature flag.

**Guarantees:** an alloc on the audio thread under
`--features debug-assert-no-alloc` panics with a clear backtrace.

### C10 — Dormant Plugin Impl

**Purpose:** preserve [`src/audio/plugin.rs`](../../../src/audio/plugin.rs)'s
`TonismPlugin: Plugin` impl as a future v0.2+ VST3 / CLAP build target.

**Boundary:** either always-compiled (with the standalone binary not
calling into it), or gated behind a `plugin-export` cargo feature.

**Guarantees:** the impl continues to consume the same `src/domain/` API
the cpal-direct path does. When it diverges from compiling, that is a
loud signal — never a silent skew. Decision between "always compile" vs
"feature-gated" is made at the end of phase B once the new path's shape
is concrete.

## Phased plan

---

Each phase **ends with a manual 5-minute clean-audio session** on the
user's primary dev machine (mic → headphones loopback) before the next
phase starts. The phase that introduces choppiness owns the bug.

### Phase A — Walking skeleton

Stand up `main.rs` as a near-copy of the cpal `feedback.rs` example shape:
duplex stream, input slice copied into a small ring, output slice drained
from the same ring. No domain code. No GUI. No params. No telemetry.

**Exit:** 5-minute clean-audio loopback on mac + win. This is the
falsifier for ADR-005's central premise; if it fails, the root cause is
below cpal and the whole pivot is wrong.

### Phase B — Domain in the callback

Replace the trivial ring copy in C2 with a call into the existing domain
chain. Hardcode parameter values (gain at 0 dB, bypass off, test-signal
off). Still no GUI, no live params.

**Exit:** 5-minute clean-audio session through `Gain::process` and any
other domain blocks. Proves C2 + the domain seam.

**Decision point at exit:** is C10 always-compiled or feature-gated?
The answer depends on whether keeping the `Plugin` impl compiling adds
friction to the new path's iteration speed. Record the call in this spec.

### Phase C — Parameter system + smoothing (C3 + C4)

Build the new parameter system. Wire C2's callback to read smoothed
values via `.next()` per frame. Still no GUI; toggle params from a small
test harness (e.g. a CLI flag or a tick-the-target test).

**Exit:** 5-minute clean-audio session with a programmatic gain ramp
(known click-free trace from the smoother). A2 enforcement (C9) is on.

### Phase D — Window stands up next to a running audio stream

Stand up C6 (egui-winit + renderer) drawing a static UI (no live params,
no telemetry reads). The window runs on its own thread; the audio stream
from phase C runs underneath unchanged.

**Exit:** 5-minute clean-audio session **with the window open and
repainting at 60 Hz**, no GUI ↔ audio coupling yet. This isolates
"adding a window breaks audio" from "GUI traffic breaks audio" — phase E
adds the traffic.

### Phase E — GUI ↔ audio param writes wired

Hook the egui update closure from today's `editor.rs` to C3's write API.
Param changes from the slider widgets now reach the audio thread through
the C3 + C4 path. xrun counter widget reads C5's existing atomic.

**Exit:** 5-minute clean-audio session with the user actively moving
sliders. No xrun increments, no audible artifacts.

### Phase F — Latency meter + test-signal re-integration

Plumb C5's `LatencyHandle` (today's `src/audio/latency.rs`) into the new
C2 callback. The "Measure latency" button in the egui editor works again.
Re-enable the 440 Hz test-signal toggle.

**Exit:** mvp-02 acceptance criteria are reachable on the new path.

### Phase G — Device picker UX + last-used persistence

C7 grows a GUI device picker (mac / win device enumeration in the egui
UI) and persists the user's last-used `(input, output, SR, buffer size)`
to a config file under the OS-conventional location. Reconnecting devices
mid-session is out of scope for the MVP; restart picks up the new
selection.

**Exit:** user can pick their device set inside the app without CLI
flags. Param values survive device switches inside one process lifetime
(the C3 persistence guarantee).

### Phase H — Cutover & cleanup

Remove the runtime use of `nih_export_standalone!` from `main.rs`. Apply
the phase-B decision to C10 (feature-gate or leave compiled). Update
[`docs/standards/architecture.md`](../../standards/architecture.md)'s
"pending decisions" section to mark **composition-root location** and the
**lock-free primitive for GUI ↔ audio messaging** as resolved.

**Exit:** PR merged to main, the MVP success bars from G5 met end-to-end.

## Risks & open questions

---

| Risk / question | Reduction strategy |
| --- | --- |
| Phase A reproduces the choppiness despite using the feedback example shape. | ADR-005 is falsified. Pivot back; restart root-cause investigation against the upstream wrapper with a known boundary. |
| The eframe-vs-egui-winit-glow-vs-custom choice for C6 introduces its own integration friction. | The decision is local to phase D and bounded; pick the simplest of the three that runs the existing `editor.rs` update closure unmodified, record the choice in this spec, move on. |
| C9 (A2 enforcement) cannot wrap a cpal callback as cleanly as `nih_plug/assert_process_allocs` wraps `Plugin::process`. | Investigate `assert_no_alloc` crate as the first candidate at the start of phase C; if it does not fit, hand-roll a thread-local guard. Either way the gate is in place before phase E ships. |
| The dormant C10 impl drifts and is uncompilable when v0.2 plugin export starts. | Phase H's feature-gate vs always-compile decision is the mitigation. Recorded in this spec at phase B exit. |
| The phase plan is too coarse and a single phase hides multiple integration steps. | Each phase has a one-line exit criterion; if implementation discovers a phase needs splitting, split it in this spec rather than hiding the substeps in the PR. |

## Test strategy

---

- **Unit tests** stay in `src/domain/` — no change. Adding the new
  parameter system + smoother grows this surface; tests are mandatory at
  the same bar as today's `src/domain/latency.rs` tests.
- **Smoke tests** (today: 3 pass at the workspace root) cover boot of the
  standalone binary. The phase-A exit ships a smoke test that opens +
  closes a cpal stream without panic, equivalent to today's
  `tonism --help` smoke.
- **Integration tests** for C2's signal flow are buffer-in / buffer-out
  tests against the domain chain — already the shape used by
  `tests/latency_meter_round_trip.rs`. Add equivalent coverage as C2
  evolves.
- **Manual 5-minute clean-audio session** is the operational gate at
  every phase exit. There is no automated substitute for this in the MVP;
  recording it as a manual step is the honest plan.
- **30-minute stress with parameter changes** is the MVP-spec G5 gate;
  it runs once at phase H, against the full assembled binary.

## References

---

- [ADR-005 — Standalone Audio Path: Direct cpal](../../adr/005-standalone-audio-cpal-direct.md)
- [ADR-002 — Standalone Runner & Audio Entry Point](../../adr/002-standalone-runner.md)
  (partially superseded by ADR-005 for the standalone path; still in force for v0.2+ plugin export)
- [ADR-004 — Reverse GUI Library Choice — Adopt egui](../../adr/004-gui-library-egui.md)
  (unchanged; the GUI library stays egui, only the adapter changes)
- [Architecture Standards](../../standards/architecture.md)
  — rules A1, A2, A4, A5, F1, F4 are the load-bearing invariants the
  pivot must preserve.
- [MVP Spec](../mvp/spec.md) — G5's success bars are the end-to-end gate.
- [cpal `feedback.rs` example](https://github.com/RustAudio/cpal/blob/master/examples/feedback.rs)
  — the known-good starting shape for phase A.
- [Windows ASIO Support Spec](../windows-asio-support/spec.md)
  — orthogonal but enabled by this pivot.
