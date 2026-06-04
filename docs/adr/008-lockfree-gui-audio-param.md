# Lock-Free GUI ↔ Audio Parameter Primitive

Owner: Z3U2

Last update: 2026-06-04

**Status:** Accepted — resolves the "lock-free primitive for GUI ↔ audio
messaging" TBD in [Architecture Standards](../standards/architecture.md)
rules F4 and A6.

## Context

---

Architecture rules F4 ("parameter changes flow GUI → audio one-way through
a lock-free channel") and A6 ("UI ↔ audio communication goes through a
lock-free channel owned by the domain") were in force from the start, but
the concrete primitive was TBD. Under the original
`nih_export_standalone!` path ([ADR-002](002-standalone-runner.md)),
nih-plug's own `Smoother` and `rtrb`-backed parameter ring owned this
responsibility; Tonism did not have to choose.

[ADR-005](005-standalone-audio-cpal-direct.md) explicitly recorded this as
architectural debt: "every alternative requires us to invent the GUI ↔
audio parameter primitive — exactly the TBD the architecture standards
explicitly defer." Phases C and C4 of the
[implementation spec](../specs/cpal-direct-standalone/spec.md) name the
contracts the new primitive must satisfy (spec § C3 and § C4).

## Decision

---

The lock-free GUI ↔ audio parameter primitive is the **atomic-based
parameter system in `src/params.rs`**, built across Phases C–H of the
cpal-direct standalone spec. It consists of three types:

### Float parameters — `FloatParamHandle` / `SmoothedFloatParam`

Each float parameter is backed by a single `Arc<AtomicU32>` whose payload
is an f32 bit-encoded via `f32::to_bits` / `f32::from_bits` (no
endianness concern; both halves run in the same process).

- **`FloatParamHandle`** — the GUI-thread write side. `Clone`able; sent
  to the GUI. `set(value)` clamps to the declared range and stores via
  `Ordering::Relaxed`. `target()` reads the most recent stored value.
  `build_smoothed()` constructs a fresh `SmoothedFloatParam` against the
  same `Arc<AtomicU32>`, used when the audio stream is (re)started.
- **`SmoothedFloatParam`** — the audio-thread read side. Not `Clone` (the
  `LinearSmoother` state is per-stream). `next()` does one `Relaxed` load
  of the atomic, calls `LinearSmoother::set_target`, then returns
  `LinearSmoother::next()`. This is the per-frame A2-clean path: one
  atomic load plus arithmetic; no allocation, no lock, no syscall.

Static parameter metadata (name, min, max, default, unit,
`smoothing_time_secs`) is allocated once at construction in a shared
`Arc<FloatParamMetadata>` and never mutated.

### Bool parameters — `BoolParam`

Bool parameters are backed by `Arc<AtomicBool>`. `BoolParam` is `Clone`
and serves as both the GUI write side (`set(value)`) and the audio read
side (`value()`). There is no smoothed variant; a single-cycle state flip
does not benefit from smoothing.

### Smoothing engine — `LinearSmoother` (`src/domain/smoother.rs`)

`LinearSmoother` is the per-frame smoothing engine used by
`SmoothedFloatParam`. It lives in `src/domain/` (A1-eligible: no imports
from audio I/O, GUI, or plugin host crates). `prepare(SampleRate)` sets
the step size from the declared `smoothing_time_secs`; `next()` advances
toward the current target by one sample. `snap_to_target()` collapses
the ramp instantly, used on stream startup so the first buffer does not
ramp from a stale previous-session value.

### All parameters — `TonismParams` / `TonismParamsAudio`

`TonismParams` (GUI shape) and `TonismParamsAudio` (audio shape) are
registry structs that collect all four parameters (`input_gain`,
`output_gain`, `bypass`, `test_signal`). `TonismParams::new(smoothing_time_secs)`
constructs both halves and returns them as a pair; the `TonismParamsAudio`
struct is moved into the cpal callback closure via `build_streams()`.

### Stream-restart persistence (spec C3 guarantee)

The `Arc<AtomicU32>` / `Arc<AtomicBool>` storage is the persistent state.
When the audio stream is torn down (device change, sample-rate change),
the `FloatParamHandle` and `BoolParam` clones held by the GUI survive
unchanged. When `build_streams()` is called for the new stream, it calls
`FloatParamHandle::build_smoothed()` on each float handle, which reads
the surviving target from the atomic and constructs a fresh
`SmoothedFloatParam` initialised to that target. `snap_to_target()` is
then called before the stream starts, so there is no ramp artefact on
reconnect. Parameter values are never lost across device switches inside
one process lifetime.

## Constraints satisfied

---

| Constraint | How it is met |
| --- | --- |
| F4 — GUI → audio one-way | `FloatParamHandle::set()` / `BoolParam::set()` store into atomics; the audio side has no write path back. |
| F1 — one authoritative copy | The `Arc<Atomic*>` is the single authoritative value; `LinearSmoother` is derived state, not a parallel mutable copy. |
| A2 — no alloc / lock / syscall on audio thread | `SmoothedFloatParam::next()` does one `Relaxed` atomic load + smoother arithmetic. `BoolParam::value()` does one `Relaxed` atomic load. No allocation path exists. |
| A6 — lock-free channel owned by the domain | The smoother lives in `src/domain/smoother.rs`; the atomic wiring lives in `src/params.rs` (infrastructure layer). Neither touches audio I/O, GUI, or plugin-host crates. |
| C3 — survives stream restart | `Arc` storage outlives the audio stream; `build_smoothed()` + `snap_to_target()` reconstructs the per-stream state from the surviving target on every stream open. |
| C4 — per-frame `.next()` semantics | `SmoothedFloatParam::next()` and `LinearSmoother::next()` advance exactly one sample per call, matching nih-plug's `Smoother` contract. |

## Consequences

---

### Positive

- The primitive is entirely first-party and auditable; there is no
  upstream wrapper whose internals need reading to understand the
  parameter flow.
- `Arc<AtomicU32>` is the simplest lock-free structure that satisfies F4
  and A2 simultaneously. The encoding (f32 bit-cast) has no endianness
  or NaN propagation surprises within a single process.
- Stream restart preserves parameter state with no additional persistence
  layer, satisfying the C3 guarantee without a database or file write.

### Negative

- There is no back-pressure or sample-accurate delivery: the GUI can write
  a new target at any time and the audio thread picks it up on its next
  `next()` call, which may be up to one block later. This is acceptable
  for the MVP (gain trims, bypass, test-signal), but would not be
  sufficient for note-level automation at sample accuracy.
- Extending the parameter set requires adding fields to both `TonismParams`
  and `TonismParamsAudio` and threading them through `build_streams()`.
  This is explicit and safe but more boilerplate than a registration-based
  system.

## Follow-ups

---

- [ADR-005](005-standalone-audio-cpal-direct.md) — the pivot that put
  this primitive in scope.
- [ADR-007](007-composition-root.md) — the composition root that wires
  `TonismParams` into the audio callbacks.
- [`docs/specs/cpal-direct-standalone/spec.md`](../specs/cpal-direct-standalone/spec.md)
  § C3, § C4 — the spec contracts this ADR implements.
