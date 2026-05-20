//! cpal-direct standalone — explicit binary entry point.
//!
//! Thin wrapper over [`tonism::cpal_direct::run`]. The same function is
//! invoked by the default `tonism` binary in `src/main.rs` when the
//! `plugin-export` feature is off (per ADR-005's C10 decision); this
//! binary exists as an unambiguous "run the cpal path regardless of
//! feature flags" entry, useful during Phase C–G iteration.
//!
//! The `#[global_allocator]` declaration mirrors the one in
//! `src/main.rs`. It is cfg-gated on `debug-assert-no-alloc`; when off,
//! the binary uses the system allocator with zero overhead.
//!
//! See:
//! - `docs/adr/005-standalone-audio-cpal-direct.md`
//! - `docs/specs/cpal-direct-standalone/spec.md` (Phase C)

// C9 global allocator. Same `not(feature = "plugin-export")` clause as
// `src/main.rs` — see the comment there.
#[cfg(all(feature = "debug-assert-no-alloc", not(feature = "plugin-export")))]
#[global_allocator]
static ALLOC_GUARD: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

fn main() -> anyhow::Result<()> {
    tonism::cpal_direct::run()
}
