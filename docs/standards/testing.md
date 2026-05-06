# Testing Standards

**Status:** Draft. Test infrastructure is not yet in place; this file states the policy the first tests will be held to.
**Last updated:** 2026-05-06

## Pyramid shape

We follow the **Testing Trophy** (Kent C. Dodds): integration is the load-bearing layer, unit tests cover domain invariants, e2e is thin but real.

For an audio product, the trophy looks like:

| Layer | What it tests | Bar |
|---|---|---|
| **e2e (full audio path)** | Real audio device → DSP graph → real audio device, headless harness driving a known input. | Catches buffer-underrun regressions, latency stack-up, glitches at parameter changes. Slow, but the only thing that defends the [success definition](../specs/product-architecture.md#definition-of-success). |
| **Integration (domain + adapter)** | A signal chain end-to-end with a fake audio device (in-memory buffer source/sink). | Catches contract drift between the chain and the audio adapter. |
| **Unit (domain)** | Pure DSP blocks, parameter math, signal-chain construction. | Boundary-value heavy; cheap and fast. |

## Rules

| ID | Rule | Tonism note |
|---|---|---|
| G1 | Skew the suite toward integration and a small set of audio-path e2e tests over a large unit suite. | |
| G2 | **No module mocks** (no monkey-patched `extern` blocks, no test-only conditional compilation that swaps real impls). Use trait-based DI; register a fake in tests via the same constructor production uses. | |
| G3 | **Test behaviour, not implementation.** Assert on processed audio output, observable state changes, emitted events — never on internal call counts. | |
| G4 | **Real dependencies over deep mocks at boundaries.** Use a real in-memory audio buffer for chain tests; do not mock `cpal`-shaped traits with elaborate spies. For preset I/O, hit a temp directory, not a fake `Filesystem` trait. | |
| G5 | **Co-located tests.** Use Rust's `#[cfg(test)] mod tests` at the bottom of the source file for unit tests; use the top-level `tests/` directory only for true integration tests that exercise the public crate API. | Rust's idiom; we follow it without restating it. |
| G6 | **Flakes are bugs.** A flaky audio-path test is quarantined, then root-caused — never retried in CI as policy. | Especially load-bearing here: realtime-audio flakes often reveal real timing bugs. |
| G7 | **Boundary-value analysis** (Glenford Myers, _The Art of Software Testing_) for domain invariants: zero, one, max buffer size; min/max sample rate; parameter range edges; off-by-one on chain length. | |

## What we do not test

- Cargo, `clippy`, or `rustfmt` behaviour — the toolchain is trusted.
- Third-party crate internals — only our use of them at the boundary.
- GUI pixel rendering — out of scope until the GUI framework is chosen, and even then, behaviour over pixels.

## Pending decisions

- The e2e harness — whether to drive it through `nih-plug`'s standalone mode or a custom test host. TBD when the plugin framework is committed.
- Property-based testing (e.g. `proptest`) — leading candidate for DSP invariants, not yet committed.
- Performance regression tests (latency, CPU, allocations on the audio thread) — covered separately under [infrastructure.md](infrastructure.md) once the budget tooling is chosen.
