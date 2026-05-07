use nih_plug::prelude::*;

/// All user-visible parameters for Tonism.
#[derive(Params)]
pub struct TonismParams {
    /// Input trim gain in dB. Range: -60 dB to +12 dB, default 0 dB.
    #[id = "input_gain"]
    pub input_gain: FloatParam,

    /// Output trim gain in dB. Range: -60 dB to +12 dB, default 0 dB.
    #[id = "output_gain"]
    pub output_gain: FloatParam,

    /// Hard bypass: when true the plugin passes audio through unmodified.
    #[id = "bypass"]
    pub bypass: BoolParam,

    /// Test-signal inject: when true a sine tone is mixed in pre-gain.
    #[id = "test_signal"]
    pub test_signal: BoolParam,
}

impl Default for TonismParams {
    fn default() -> Self {
        let gain_range = FloatRange::Linear {
            min: -60.0,
            max: 12.0,
        };
        Self {
            input_gain: FloatParam::new("Input Gain", 0.0, gain_range)
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_unit(" dB")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            output_gain: FloatParam::new("Output Gain", 0.0, gain_range)
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_unit(" dB")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            bypass: BoolParam::new("Bypass", false),
            test_signal: BoolParam::new("Test Signal", false),
        }
    }
}
