/// Audio sample rate in Hz (e.g. 44_100, 48_000).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleRate(pub u32);

/// Audio buffer size in frames (e.g. 128, 512).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferSize(pub u32);

/// Gain expressed in decibels (dBFS).  0 dB = unity gain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decibels(pub f32);

/// Gain expressed as a linear amplitude multiplier.  1.0 = unity gain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GainLinear(pub f32);

impl From<Decibels> for GainLinear {
    fn from(db: Decibels) -> Self {
        GainLinear(10f32.powf(db.0 / 20.0))
    }
}
