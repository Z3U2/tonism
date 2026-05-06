# Domain Standards

**Status:** Draft. Principles only — no domain modules exist yet. The first DSP block, signal-chain type, and parameter model will be committed against this file.
**Last updated:** 2026-05-06

## Principles

The domain is the [Signal chain](../specs/product-architecture.md#product-layers) and [Tone state](../specs/product-architecture.md#product-layers) layers: pure types, pure functions, no I/O. Rust's type system is the first line of defence; runtime checks are reserved for true external boundaries.

### Object / function design

| ID | Rule | Tonism note |
|---|---|---|
| C1 | **Tell, don't ask.** Operate on a domain value through its own methods; don't pull state out and decide externally. | |
| C2 | **Law of Demeter.** No `chain.blocks[0].param.value` reaches across layers. | |
| C3 | **Immutability by default.** Mutate only where the realtime path demands it; clearly mark those types. | The audio thread will mutate per-buffer scratch state; everything else is `&self`. |
| C4 | Composition over inheritance — Rust traits + concrete types, no shared-state via inheritance pattern (we don't have that anyway). | |
| C5 | **Value objects** for domain primitives. NewType wrappers for `SampleRate`, `BufferSize`, `BlockId`, `ParamId`, `Decibels`, `Hertz` — never raw `f32`/`u32`/`String`. | The type system rules out swapping a sample rate with a buffer size. |
| C6 | **Fail fast at boundaries.** Validate external input (preset files, MIDI, plugin host parameters) once in the adapter; the core trusts what it receives. | |

### Errors

The domain follows **railway-oriented programming** (Scott Wlaschin, _Domain Modeling Made Functional_): expected failures are values, not exceptions.

| ID | Rule | Tonism note |
|---|---|---|
| D1 | Use `Result<T, E>` for expected failures. `panic!` only for true bugs (broken invariants). | Rust's idiom; we don't need a "no exceptions" rule, but we do need discipline about not `unwrap()`-ing in domain code. |
| D2 | **Distinct error taxonomies** for domain vs infrastructure. A `PresetParseError` is not the same type as an `IoError`. | Adapters translate infrastructure errors into domain errors at the boundary. |
| D3 | No silent error swallowing. Every `Err` is either handled, mapped with context, or propagated with `?`. | |
| D4 | Errors carry context — original cause and enough detail to diagnose without reproducing. | `thiserror` (or equivalent) is the leading candidate; not yet committed. |

### Data, types, contracts

| ID | Rule | Tonism note |
|---|---|---|
| E1 | **DTOs at the edge, domain models inside.** Preset files, plugin-host parameter blobs, and MIDI messages are parsed into domain types in the adapter. | |
| E2 | **Parse, don't validate.** Use a schema-aware parser at the boundary (e.g. `serde` + a validating constructor); inside the domain, the type carries the guarantee. | |
| E3 | No `Box<dyn Any>`, no untagged `serde_json::Value` in domain code. | |
| E4 | **Discriminated unions for state.** Rust enums for any "one of N" — chain state, parameter status, processing mode. Avoid bool soup. | |

## What "domain" excludes

- Audio device enumeration and selection — infrastructure.
- Plugin-host parameter automation wiring — infrastructure (adapter).
- GUI widget state — presentation.
- Filesystem layout for presets — infrastructure.

## Pending decisions

- Error library: `thiserror`, `anyhow`, or hand-rolled — TBD.
- Whether the parameter model is owned by the domain or by the plugin framework adapter (depends on plugin-framework choice).
- The exact NewType set; will grow as DSP blocks land.
