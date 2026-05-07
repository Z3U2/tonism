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
- `src/gui/` — vizia editor
- `tests/` — integration tests (smoke + boundary fixtures)
- `docs/` — specs, ADRs, standards

## Architecture decisions

See [docs/adr/](docs/adr/) for the load-bearing decisions:

- [ADR-001](docs/adr/001-language-choice.md) — Rust + nih-plug + cpal stack
- [ADR-002](docs/adr/002-standalone-runner.md) — nih-plug standalone as audio entry point
- [ADR-003](docs/adr/003-gui-library.md) — `nih_plug_vizia` via BillyDM fork
