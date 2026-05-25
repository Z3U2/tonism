# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Tonism is a standalone real-time guitar processor in Rust. It captures audio input, runs it through a DSP signal chain, and outputs processed audio — all with hard real-time constraints (<10 ms latency, zero underruns).

Check `docs/specs/` for current implementation status.

## Build and test commands

```bash
cargo build                              # debug build
cargo build --release                    # release (use for audio work — debug is too slow for real-time)
cargo run                                # run standalone with eframe GUI
cargo run --release                      # run standalone, release mode
cargo test                               # run all tests
cargo test <test_name>                   # run a single test
cargo test --features plugin-export      # test including dormant nih-plug surface
cargo clippy --all-targets -- -D warnings  # lint (runs in pre-commit hook)
cargo fmt --check                        # format check (runs in pre-commit hook)
```

### Feature flags

- **`plugin-export`** — compiles the dormant nih-plug Plugin impl + nih_plug_egui GUI adapter. Off by default; the standalone runs cpal-direct. Enable to exercise the v0.2+ VST3/CLAP surface.
- **`debug-assert-no-alloc`** — wraps audio callbacks in a runtime assertion that panics on any heap allocation. Use during development: `cargo run --features debug-assert-no-alloc`. Works in both debug and release builds.

### Pre-commit hooks

Managed by `lefthook` (`brew install lefthook && lefthook install`). Runs `cargo fmt --check` and `cargo clippy` on staged `.rs` files.

### Commit style

Conventional Commits: `feat(audio): add bypass toggle`, `fix(gui): render labels in dark mode`.

## Architecture

Hexagonal architecture with functional core / imperative shell. The domain is pure (no I/O); adapters handle audio devices and GUI at the edges.

### Module map

- **`src/domain/`** — Pure DSP core. Zero imports from audio, GUI, or filesystem (rule A1). Contains:
  - `process.rs` — `Process` trait: the lifecycle contract for all DSP blocks (`prepare` → `reset` → `process`)
  - `blocks/` — concrete DSP blocks (e.g. `Gain`)
  - `types.rs` — NewType wrappers (`SampleRate`, `BufferSize`, `Decibels`, `GainLinear`). Inner fields are private; use `::new()` and `.value()` accessors, never `.0`.
  - `smoother.rs` — `LinearSmoother` for parameter smoothing
  - `latency.rs` — pure latency measurement algorithm

- **`src/cpal_direct.rs`** — Composition root (C8). Builds the audio streams, wires params, and launches the GUI. Two entries: `run_gui()` (eframe window) and `run()` (headless, stdin-blocking).

- **`src/params.rs`** — Lock-free parameter system for the standalone path. Split design: `FloatParamHandle`/`BoolParamHandle` (GUI thread, cloneable, atomic stores) and `SmoothedFloatParam`/`BoolParamReader` (audio thread, reads same atomic + private smoother). No locks, no allocations on the audio thread.

- **`src/audio/`** — Audio infrastructure:
  - `latency.rs` — `LatencyMeter` + `LatencyHandle` for loopback measurement
  - `xrun.rs` — `XrunCounter` (shared atomic between audio and GUI)
  - `log_bridge.rs` — lock-free audio-to-log queue (audio thread never logs inline)
  - `plugin.rs`, `params.rs` — dormant nih-plug adapter (gated behind `plugin-export`)

- **`src/gui/`** — GUI layer:
  - `standalone.rs` — eframe app for the cpal-direct path (always compiled)
  - `editor.rs` — nih_plug_egui editor (gated behind `plugin-export`)

### Real-time rules (A2)

The audio callback must never allocate, lock, or syscall. This is the single most important constraint. Violations cause audible glitches.

- Parameter changes flow one-way: GUI → atomic store → audio thread reads + smooths (F4)
- Audio thread communicates out via lock-free ring buffers (rtrb), never direct writes to GUI state
- `debug-assert-no-alloc` feature enforces this at runtime

### Data flow

```
GUI thread                          Audio thread
    │                                    │
    ├─ FloatParamHandle::set() ──→ AtomicU32 ──→ SmoothedFloatParam::next()
    │                                    │
    ├─ reads XrunCounter (atomic) ←──────┤ bumps on ring over/underflow
    │                                    │
    └─ reads LatencyHandle::state() ←────┘ writes capture buffer
```

## Testing

Testing Trophy: integration-heavy, unit tests for domain invariants, thin e2e.

- `tests/` — integration tests (public crate API). e.g. `tests/latency_meter_round_trip.rs`
- `#[cfg(test)] mod tests` at bottom of source files — unit tests for domain logic
- Test behavior, not implementation (G3). Assert on processed audio output, not internal call counts.
- Use real dependencies over mocks (G4). In-memory audio buffers, temp directories — not elaborate spies.

## Key references

- `docs/standards/architecture.md` — rules A1-A6, F1-F4 (hexagonal, real-time, concurrency)
- `docs/standards/domain.md` — rules C1-C6, D1-D4 (NewTypes, errors, tell-don't-ask)
- `docs/standards/testing.md` — rules G1-G7 (testing trophy, no mocks, boundary values)
- `docs/standards/infrastructure.md` — rules I1-I5, J1-J4 (commits, perf budgets)
- `docs/adr/` — architecture decision records (language, GUI library, cpal-direct pivot)
- `docs/specs/cpal-direct-standalone/` — current implementation spec and phase tracker
