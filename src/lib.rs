pub mod audio;
pub mod cpal_direct;
pub mod domain;
pub mod params;

// C10 per ADR-005: the GUI is currently a nih_plug_egui adapter; gated
// behind `plugin-export` until the cpal-direct path grows its own egui
// host (Phase D+).
#[cfg(feature = "plugin-export")]
pub mod gui;
