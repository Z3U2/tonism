//! Tonism standalone binary.
//!
//! The `#[global_allocator]` declaration is cfg-gated on
//! `debug-assert-no-alloc` — when off, the binary uses the system
//! allocator with zero overhead.
//!
//! Dispatches between two entry paths at compile time per ADR-005's C10
//! decision:
//!
//! - **Default** (no features): the cpal-direct standalone path with an
//!   eframe GUI window. [`tonism::cpal_direct::run_gui`] opens the audio
//!   streams + a native window; closing the window tears down the session.
//!   For headless iteration, `cargo run --bin feedback` runs the stdin-
//!   blocking variant.
//! - **`--features plugin-export`**: routes to nih-plug's standalone
//!   wrapper via `nih_export_standalone!`. Exists so the v0.2+ VST3 /
//!   CLAP-ready surface stays exercised and CI catches drift; not the
//!   primary user-facing path.
//!
//! See:
//! - `docs/adr/005-standalone-audio-cpal-direct.md` (C10 decision)
//! - `docs/specs/cpal-direct-standalone/spec.md` (Phases D–H)

// C9 global allocator. When `plugin-export` is also on, nih-plug's own
// `assert_process_allocs` feature installs its own global allocator;
// declaring ours alongside would cause a duplicate-global-allocator link
// error. The `not(feature = "plugin-export")` clause yields to nih-plug
// in that combined configuration. (The cpal-direct callback path is
// still wrapped in `assert_no_alloc_audio` either way; only the global
// allocator hook differs.)
#[cfg(all(feature = "debug-assert-no-alloc", not(feature = "plugin-export")))]
#[global_allocator]
static ALLOC_GUARD: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("tonism=info,nih_plug=warn")
            }),
        )
        .init();
    run()
}

/// Plugin-export entry: hand control to nih-plug's standalone wrapper.
/// Kept as a parallel target so the `Plugin` impl in
/// `src/audio/plugin.rs` stays compiling and exercisable.
#[cfg(feature = "plugin-export")]
fn run() -> anyhow::Result<()> {
    use nih_plug::prelude::nih_export_standalone;
    use tonism::audio::plugin::TonismPlugin;
    nih_export_standalone::<TonismPlugin>();
    Ok(())
}

/// Default entry: the cpal-direct standalone path with an eframe GUI
/// window. `cargo run --bin feedback` runs the headless variant.
#[cfg(not(feature = "plugin-export"))]
fn run() -> anyhow::Result<()> {
    tonism::cpal_direct::run_gui()
}
