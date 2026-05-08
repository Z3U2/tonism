/// Audio sample rate in Hz (e.g. 44_100.0, 48_000.0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleRate(f32);

impl SampleRate {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }
    pub fn value(self) -> f32 {
        self.0
    }
}

impl Default for SampleRate {
    fn default() -> Self {
        Self(44_100.0)
    }
}

/// Audio buffer size in frames (e.g. 128, 512).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferSize(u32);

impl BufferSize {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub fn value(self) -> u32 {
        self.0
    }
}

impl Default for BufferSize {
    fn default() -> Self {
        Self(512)
    }
}

/// Gain expressed in decibels (dBFS).  0 dB = unity gain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decibels(f32);

impl Decibels {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }
    pub fn value(self) -> f32 {
        self.0
    }
}

impl Default for Decibels {
    fn default() -> Self {
        Self(0.0)
    }
}

/// Gain expressed as a linear amplitude multiplier.  1.0 = unity gain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GainLinear(f32);

impl GainLinear {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }
    pub fn value(self) -> f32 {
        self.0
    }
}

impl Default for GainLinear {
    fn default() -> Self {
        Self(1.0)
    }
}

impl From<Decibels> for GainLinear {
    fn from(db: Decibels) -> Self {
        GainLinear(10f32.powf(db.value() / 20.0))
    }
}
