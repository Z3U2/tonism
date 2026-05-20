pub mod backend;
pub mod latency;
pub mod log_bridge;
pub mod xrun;

// C10 per ADR-005: the nih-plug Plugin impl + its TonismParams alias
// stay dormant in default builds. Compile + link them with
// `--features plugin-export` to exercise the v0.2+ VST3/CLAP-ready
// surface (also gated in CI to catch silent drift).
#[cfg(feature = "plugin-export")]
pub mod params;
#[cfg(feature = "plugin-export")]
pub mod plugin;
