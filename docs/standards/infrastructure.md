# Infrastructure Standards

**Status:** Draft. Most tooling not yet chosen. This file states the standards each tool will be picked against; concrete tools land here as decisions are made.
**Last updated:** 2026-05-06

## Tooling & process

| ID | Rule | Tonism note |
|---|---|---|
| I1 | **Conventional Commits**, English. | Already in force — see [commit-style.md](commit-style.md). |
| I2 | **Tiered checks.** Pre-commit runs fast local checks (`cargo fmt --check`, `cargo clippy` on changed files). Pre-push runs `cargo test`. CI runs everything including the audio-path e2e suite. | Concrete hook configuration TBD. |
| I3 | **Machine-checked architecture rules.** The dependency rule (domain has no I/O imports — see [architecture.md](architecture.md)) is enforced by the build, not by review. | Tool TBD: leading candidates are `cargo-deny` for crate-graph rules, custom `clippy` lints, or a workspace-level dependency check. |
| I4 | **Reproducible builds.** `Cargo.lock` is committed. Toolchain is pinned in `rust-toolchain.toml`. CI uses the same toolchain. | |
| I5 | **One command each** to run, test, ship. `cargo run` and `cargo test` cover the first two; the release pipeline (which produces VST3, CLAP, and standalone artefacts per [ADR-001](../adr/001-language-choice.md)) is TBD. | A `Justfile` or `Makefile` is the leading candidate as a thin wrapper once non-trivial flag combinations appear. |

## Performance & operability

The product's success bars are operational ([< 10 ms latency, zero underruns over 5 min, crash-free 30 min](../specs/product-architecture.md#definition-of-success)). The standards here defend those bars.

| ID | Rule | Tonism note |
|---|---|---|
| J1 | **Performance budgets are explicit and measured.** Round-trip latency (p50/p95), buffer-underrun count, audio-thread CPU headroom, allocations-per-buffer (target: 0). | The budget numbers live in [okrs/q1.md](../okrs/q1.md). The measurement tooling is TBD. |
| J2 | **Structured logging at I/O boundaries**, with correlation. The audio thread does **not** log inline — it emits to a lock-free queue drained by a non-realtime thread. | `tracing` is the leading Rust crate; not yet committed. |
| J3 | **Feature flags for risky changes** carry an expiry. | Premature for MVP; will gain teeth once we have releases that ship to anyone but the author. |
| J4 | **Observability before optimisation.** Measure on the dev machine first, then cut. | |

## Release & artefacts

**TBD.** Once the plugin framework is committed, this section names the build matrix (VST3 / CLAP / standalone × macOS / Windows / Linux), the signing/notarisation policy (relevant for macOS), and the release-cadence policy.

## Out of scope (for now)

- Hosted services / cloud sync — see [non-goals](../specs/product-architecture.md#non-goals).
- Crash reporting and telemetry — privacy-affecting; deferred until there are non-author users.
- Public CI dashboards — the project has one developer.

## Pending decisions

- CI provider.
- Pre-commit hook runner (e.g. `lefthook`, `pre-commit`, or a hand-rolled `git` hook).
- Release pipeline and artefact signing.
- Logging crate.
- Performance-budget enforcement (criterion benchmarks gated in CI vs. local-only).
