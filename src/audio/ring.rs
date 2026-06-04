//! Audio ring buffer for input→output transport.

use rtrb::RingBuffer;

use crate::audio::latency::CAPTURE_LEN;

/// Delay between input and output, in milliseconds. Absorbs clock drift
/// between input and output devices that are not running on the same
/// hardware clock. Matches the upstream `cpal/examples/feedback.rs`
/// default; will shrink in Phase G when device-pair / aggregate-device
/// assumptions land.
pub const LATENCY_MS: f32 = 150.0;

/// Pre-filled ring buffer that transports audio from the input callback
/// to the output callback with [`LATENCY_MS`] of buffered latency.
pub struct AudioRing {
    pub producer: rtrb::Producer<f32>,
    pub consumer: rtrb::Consumer<f32>,
    pub latency_frames: usize,
}

impl AudioRing {
    /// Build a new ring buffer sized for `sample_rate` and `channels`.
    ///
    /// The ring capacity is 2× the latency pre-fill so producers always
    /// have headroom. The pre-fill pads the consumer side with silence
    /// equal to `LATENCY_MS` worth of interleaved samples.
    ///
    /// # Panics
    ///
    /// Panics if [`CAPTURE_LEN`] is not larger than `latency_frames`,
    /// which would make the latency meter unable to capture the echo.
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let channels_usize = channels as usize;
        let latency_frames = (LATENCY_MS / 1_000.0) * sample_rate as f32;
        assert!(
            CAPTURE_LEN > latency_frames as usize,
            "CAPTURE_LEN ({CAPTURE_LEN}) must exceed ring pre-fill ({} frames) \
             so the latency meter can capture the echo",
            latency_frames as usize,
        );
        let latency_samples = latency_frames as usize * channels_usize;
        let (mut producer, consumer) = RingBuffer::<f32>::new(latency_samples * 2);
        for _ in 0..latency_samples {
            producer
                .push(0.0)
                .expect("ring has 2× headroom for the pre-fill");
        }
        Self {
            producer,
            consumer,
            latency_frames: latency_frames as usize,
        }
    }
}
