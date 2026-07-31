//! Pure buffer utilities for interleaved audio data.
//!
//! These operate on raw `&[f32]` / `&mut [f32]` slices with no I/O, no
//! allocation, and no dependency on audio infrastructure (A1-clean).

/// Copy one channel out of an interleaved buffer into `out`.
///
/// `interleaved` has `channels` samples per frame. This extracts
/// `channel` (0-indexed) into `out[0..n_frames]` where
/// `n_frames = interleaved.len() / channels`.
///
/// # Panics
///
/// Panics if `out.len() < n_frames` or `channel >= channels`.
pub fn deinterleave_channel(interleaved: &[f32], channel: usize, channels: usize, out: &mut [f32]) {
    assert!(
        channel < channels,
        "channel {channel} >= channels {channels}"
    );
    let n_frames = interleaved.len() / channels;
    for i in 0..n_frames {
        out[i] = interleaved[i * channels + channel];
    }
}

/// Write one channel back into an interleaved buffer from `source`.
///
/// Inverse of [`deinterleave_channel`]: copies `source[0..n_frames]`
/// into the `channel` lane of `interleaved`.
///
/// # Panics
///
/// Panics if `source.len() < n_frames` or `channel >= channels`.
pub fn interleave_channel(
    interleaved: &mut [f32],
    channel: usize,
    channels: usize,
    source: &[f32],
) {
    assert!(
        channel < channels,
        "channel {channel} >= channels {channels}"
    );
    let n_frames = interleaved.len() / channels;
    for i in 0..n_frames {
        interleaved[i * channels + channel] = source[i];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deinterleave_stereo_ch0() {
        let interleaved = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 3 frames, 2 ch
        let mut out = [0.0f32; 3];
        deinterleave_channel(&interleaved, 0, 2, &mut out);
        assert_eq!(out, [1.0, 3.0, 5.0]);
    }

    #[test]
    fn deinterleave_stereo_ch1() {
        let interleaved = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut out = [0.0f32; 3];
        deinterleave_channel(&interleaved, 1, 2, &mut out);
        assert_eq!(out, [2.0, 4.0, 6.0]);
    }

    #[test]
    fn interleave_round_trips() {
        let original = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut out = [0.0f32; 3];
        deinterleave_channel(&original, 0, 2, &mut out);
        // Modify ch0
        for s in out.iter_mut() {
            *s *= 2.0;
        }
        let mut result = original;
        interleave_channel(&mut result, 0, 2, &out);
        assert_eq!(result, [2.0, 2.0, 6.0, 4.0, 10.0, 6.0]);
    }

    #[test]
    fn mono_deinterleave_is_copy() {
        let interleaved = [1.0, 2.0, 3.0];
        let mut out = [0.0f32; 3];
        deinterleave_channel(&interleaved, 0, 1, &mut out);
        assert_eq!(out, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn empty_buffer_no_panic() {
        let interleaved: [f32; 0] = [];
        let mut out: [f32; 0] = [];
        deinterleave_channel(&interleaved, 0, 2, &mut out);
        interleave_channel(&mut [], 0, 2, &[]);
    }

    #[test]
    #[should_panic(expected = "channel 2 >= channels 2")]
    fn deinterleave_oob_channel_panics() {
        let interleaved = [1.0, 2.0];
        let mut out = [0.0f32; 1];
        deinterleave_channel(&interleaved, 2, 2, &mut out);
    }
}
