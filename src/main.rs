use nih_plug::prelude::*;
use tonism::audio::plugin::TonismPlugin;

fn main() {
    nih_export_standalone::<TonismPlugin>();
}
