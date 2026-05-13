# Architecture Standards

**Status:** Draft — most concrete choices still pending. Only the language (Rust, see [ADR-001](../adr/001-language-choice.md)) is fixed. This file states the architectural principles the codebase will be held to; specific crates, modules, and boundaries are filled in as decisions land.
**Last updated:** 2026-05-06

## Principles

The codebase follows **hexagonal architecture** (Alistair Cockburn) with a **functional core / imperative shell** split (Gary Bernhardt). The domain is pure and free of I/O; adapters handle the audio device, the plugin host, the GUI, and the filesystem at the edges.

| ID | Rule | Tonism-specific note |
|---|---|---|
| A1 | Domain has zero imports from audio I/O, GUI, plugin host, or filesystem crates. | The audio callback is a shell, not a core. |
| A2 | The realtime audio thread is the most stringent shell: no allocation, no locking, no syscalls inside the per-buffer path. | Violations cause underruns — the primary failure mode for the [Capture](../specs/product-architecture.md#product-layers) and [Render](../specs/product-architecture.md#product-layers) layers. |
| A3 | Dependencies point inward. Presentation (GUI) and Infrastructure (audio I/O, persistence) depend on Domain (DSP, signal-chain model); Domain depends on neither. | |
| A4 | External dependencies are reached only through traits declared in the domain. | Lets us swap the GUI framework, plugin framework, or audio backend without touching the DSP core. |
| A5 | One composition root. DI wiring lives in a single place at startup. | TBD where — depends on plugin-framework choice. |
| A6 | No layer-skipping. The GUI does not poke audio I/O directly; the audio thread does not call into the GUI. | UI ↔ audio communication goes through a lock-free channel owned by the domain. |

## Crate / module layout

**TBD.** No decision yet on whether the project is a single crate, a workspace, or split by plugin-framework convention. When chosen, this section names the crates and the dependency rule that the build system enforces (rule I3 in [best-practices](../../.claude/skills/legacy-migration/best-practices.md)).

## State & concurrency

| ID | Rule |
|---|---|
| F1 | Single source of truth per piece of tone state. The audio thread reads a snapshot; the GUI mutates an authoritative copy. No parallel mutable state. |
| F2 | Derived state (e.g. visual meters, computed coefficients) is recomputed from the source, not stored alongside it. |
| F4 | Parameter changes flow GUI → audio thread one-way through a lock-free channel; the audio thread does not write back into shared mutable state. |

## Pending decisions

These are referenced as `TBD` throughout this file and will become ADRs as they're resolved:

- Plugin framework (nih-plug is the leading candidate per [ADR-001](../adr/001-language-choice.md), not yet committed).
- Crate / workspace layout.
- Composition-root location.
- The exact lock-free primitive for GUI ↔ audio messaging.
