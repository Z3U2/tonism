# Tonism

A standalone real-time guitar processor in Rust.

> Status: pre-MVP scaffolding. See [docs/specs/mvp/spec.md](docs/specs/mvp/spec.md) for the v0.1 acceptance criteria and timebox.

## Prerequisites

- Rust stable (pinned in [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` installs it automatically on first use)
- [`lefthook`](https://github.com/evilmartians/lefthook) for pre-commit hooks — not installed by cargo: `brew install lefthook` on macOS

## Quickstart

```bash
# Install hooks (run once after clone)
lefthook install

# Build and run the standalone audio app
cargo run

# Run the test suite
cargo test
```

## macOS first-run

The standalone binary captures guitar input via CoreAudio. On first launch,
macOS prompts for microphone access — grant it via **System Settings →
Privacy & Security → Microphone**. Without this permission CoreAudio
delivers silence on the input stream.

> **Forward-looking:** when the standalone is later wrapped as a `.app` bundle
> (post-MVP), add `NSMicrophoneUsageDescription` to its `Info.plist` so the
> system prompt names the application. Week-1 builds run as a plain binary and
> rely on the system-level grant above.

## Project layout

- `src/domain/` — pure DSP types and traits ([no I/O imports — rule A1](docs/standards/architecture.md))
- `src/audio/` — nih-plug adapter, parameter set, xrun counter, audio-to-log bridge
- `src/gui/` — egui editor (see [ADR-004](docs/adr/004-gui-library-egui.md))
- `tests/` — integration tests (smoke + boundary fixtures)
- `docs/` — specs, ADRs, standards

## Profiling

Layered against the realtime constraints in [`docs/standards/architecture.md`](docs/standards/architecture.md) (rule A2: no alloc / lock / syscall in the audio callback) and [`docs/standards/infrastructure.md`](docs/standards/infrastructure.md) (J1: zero allocations per buffer).

### Layer 1 — `debug-assert-no-alloc` (regression-safe, no Xcode needed)

A Cargo feature that wraps the audio callback in a runtime assertion: any heap allocation reached from `process()` panics immediately with the offending stack trace. Deterministic — sampling profilers can miss small allocations between samples.

```bash
# Run the standalone with the assertion enabled.  Use during development.
cargo run --features debug-assert-no-alloc

# Same for the test suite.
cargo test --features debug-assert-no-alloc
```

The feature toggles `nih_plug/assert_process_allocs` underneath. Leave it OFF for release builds — the wrapper has overhead.

### Layer 2 — `cargo-instruments` (deep CPU / allocation profiling)

For ad-hoc CPU and allocation profiling on macOS. **Requires full Xcode** (not just Command Line Tools) for Instruments.app:

```bash
# One-time setup.  Install Xcode from the Mac App Store, then:
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
cargo install cargo-instruments

# Profile.  Opens the trace in Instruments.app when done.
cargo instruments -t time            # CPU time profile
cargo instruments -t alloc           # Allocations + retain/release
cargo instruments --release -t time  # Release-mode hot-path analysis
```

Note: while Tonism's standalone window is open, Instruments samples the live audio path. Play guitar through it for the duration of the run.

## Architecture decisions

See [docs/adr/](docs/adr/) for the load-bearing decisions:

- [ADR-001](docs/adr/001-language-choice.md) — Rust + nih-plug + cpal stack
- [ADR-002](docs/adr/002-standalone-runner.md) — nih-plug standalone as audio entry point
- [ADR-003](docs/adr/003-gui-library.md) — _superseded by ADR-004_
- [ADR-004](docs/adr/004-gui-library-egui.md) — `nih_plug_egui` from BillyDM's fork
