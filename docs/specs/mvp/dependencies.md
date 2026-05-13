# Tech Lead Dependencies — MVP: Glitch-free real-time guitar signal path

> Derived from [spec.md](spec.md). To be resolved by the Tech Lead **before** devs start implementation.

## Already covered by standards

- Hexagonal split, domain has no I/O imports: A1, A3, A4 in [docs/standards/architecture.md](../../standards/architecture.md)
- No alloc / locks / syscalls in audio callback: A2 in [docs/standards/architecture.md](../../standards/architecture.md)
- GUI → audio one-way lock-free channel, single source of truth: F1, F4 in [docs/standards/architecture.md](../../standards/architecture.md)
- NewType wrappers for `SampleRate`, `BufferSize`, `Decibels`: C5 in [docs/standards/domain.md](../../standards/domain.md)
- Railway-oriented errors, no `unwrap` in domain: D1–D4 in [docs/standards/domain.md](../../standards/domain.md)
- Performance budgets (latency, xruns, allocations/buffer = 0): J1 in [docs/standards/infrastructure.md](../../standards/infrastructure.md)
- Audio thread emits to lock-free queue drained off-RT: J2 in [docs/standards/infrastructure.md](../../standards/infrastructure.md)
- Trophy shape: integration over fake audio device, unit on domain, e2e on real path: G1–G7 in [docs/standards/testing.md](../../standards/testing.md)
- Conventional Commits in English: [docs/standards/commit-style.md](../../standards/commit-style.md), I1 in infrastructure
- `Cargo.lock` committed, toolchain pinned in `rust-toolchain.toml`: I4 in infrastructure

## Dependencies

- Standalone runner: **nih-plug standalone** — single path to plugin export later (alt: raw `cpal` + `winit`)
- GUI framework: **`nih_plug_egui`** from BillyDM's nih-plug fork — co-located with the chosen runner; one-line per-frame state reads instead of vizia's `SyncSignal`/`Timer`/`Memo` scaffolding (see [ADR-004](../../adr/004-gui-library-egui.md), supersedes [ADR-003](../../adr/003-gui-library.md)) (alt: `vizia_plug`)
- Audio I/O: **`cpal`** via nih-plug — CoreAudio/JACK/WASAPI covered (alt: direct `cpal`)
- Lock-free GUI ↔ audio primitive: **`triple_buffer`** for parameter snapshot — bounded, RT-safe (deferred for MVP — nih-plug params suffice; revisit for v0.2 multi-block snapshots) (alt: SPSC `ringbuf` of param events)
- Logging crate: **`tracing`** + `tracing-subscriber` — standards' leading candidate (alt: `log` + `env_logger`)
- Error library: **`thiserror`** in domain, `anyhow` at shell — standards' leading candidate (alt: hand-rolled enums)
- Crate layout: **single crate**, modules `domain` / `audio` / `gui` — week-1 solo scope (alt: cargo workspace)
- Composition root: **`main.rs`** wires standalone, GUI, DSP — the only entry point (alt: dedicated `app::bootstrap` module)
- Toolchain pin: **stable, latest** in `rust-toolchain.toml` — pin once at start (alt: nightly for unstable features)
- macOS dev setup: **microphone permission** for audio capture — required by CoreAudio (alt: instruct user to grant manually)
- Pre-commit hook: **`lefthook`** runs `fmt --check` + `clippy` on changed files — implements I2 (alt: hand-rolled `git` hook)
- Architecture-rule enforcement: **defer to v0.2** — single crate makes A1 self-evident in MVP (alt: `cargo-deny` rule on domain)

## Patterns

- Parameter flow: GUI writes new snapshot to `triple_buffer`; audio reads latest in `process()` — implements F4 (alt: SPSC ring of `(ParamId, value)` messages)
- xrun counter: `Arc<AtomicU64>` incremented in callback's underflow branch, read by GUI loop — A2-safe (alt: `cpal::StreamError::InputUnderflow` bridged to GUI bus)
- Latency measurement: inject impulse via test-signal toggle, cross-correlate loopback to locate echo, display ms — round-trip per AC1 (alt: fixed-period click + phase-delay)
- Bypass: atomic `bool` read once per buffer; when true, callback copies input → output without invoking DSP — branch outside hot loop (alt: bypass as a stateless DSP block)
- DSP block trait: `Process { fn process(&mut self, &mut [f32]); fn reset(&mut self, sr: SampleRate); }` — minimal contract for "one block", grows to chain in v0.2 (alt: free-standing per-block functions)
- Audio → log bridge: callback pushes to bounded SPSC; subscriber thread drains — implements J2 (alt: `tracing-appender` non-blocking writer)
- NewType set for MVP: `SampleRate(u32)`, `BufferSize(u32)`, `Decibels(f32)`, `GainLinear(f32)` — covers gain knobs + audio config (alt: defer until block #2)

## Test data

- Fake audio device: `AudioBackend` trait with `CpalBackend` and `BufferBackend` impls, swapped via DI — implements G2/G4 (alt: parameterise on a `Sample` source)
- Latency reference signal: 1024-sample Kronecker impulse at session sample rate — clean cross-correlation peak (alt: 1 kHz sine burst)
- 5-min silent fixture (AC2): in-memory zero-buffer source through `BufferBackend` — proves idle path is xrun-free (alt: pre-recorded silent WAV)
- Stress automation (AC3): deterministic schedule (gain ramp every 2 s, bypass toggle every 5 s) over 30 min — reproducible vs. manual (alt: documented manual protocol)
- Boundary buffer sizes: 32, 64, 128, 256, 512, 1024, 2048 — covers G7 and likely cpal block sizes (alt: 64 + 1024 only)
- Boundary sample rates: 44 100, 48 000, 88 200, 96 000 Hz — gigging-rig norms (alt: 48 000 Hz only)
