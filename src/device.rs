//! Device enumeration and config resolution for the tonism standalone app.
//!
//! Provides GUI-presentable device info ([`DeviceInfo`]) and startup config
//! resolution ([`resolve_initial_config`]) with a CLI > saved-config > system
//! default fallback chain.

use std::str::FromStr;

use anyhow::Context;
use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait};

use crate::config::TonismConfig;

/// Standard sample rates that tonism presents in the GUI.
const STANDARD_SAMPLE_RATES: &[u32] = &[44100, 48000, 88200, 96000, 176400, 192000];

/// Powers-of-two buffer sizes offered in the GUI (in addition to `None` = Default).
const POW2_BUFFER_SIZES: &[u32] = &[32, 64, 128, 256, 512, 1024, 2048];

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// GUI-presentable information about a single audio device.
///
/// Constructed by [`enumerate_devices`]. The embedded [`cpal::Device`] is kept
/// so that streams can be built from it later without a second lookup.
pub struct DeviceInfo {
    /// Human-readable name from [`cpal::DeviceDescription::name`].
    pub name: String,
    /// Serialisable device identity string (`HostId:device_name`).
    pub id_string: String,
    /// The underlying cpal device, retained for stream construction.
    pub device: cpal::Device,
    /// Standard sample rates supported by this device (intersection of all
    /// F32/I16 config ranges with [`STANDARD_SAMPLE_RATES`]).
    pub supported_sample_rates: Vec<u32>,
    /// Minimum and maximum buffer size (frames) reported by the device, or
    /// `None` if the device only reports [`cpal::SupportedBufferSize::Unknown`].
    pub buffer_size_range: Option<(u32, u32)>,
}

/// A fully resolved set of audio stream parameters, ready for use by the
/// cpal stream builder.
pub struct ResolvedDeviceConfig {
    pub input_device: cpal::Device,
    pub output_device: cpal::Device,
    /// Agreed sample rate in Hz.
    pub sample_rate: u32,
    /// Buffer size in frames, or `None` meaning [`cpal::BufferSize::Default`].
    pub buffer_size: Option<u32>,
    /// Number of channels (taken from the default output config).
    pub channels: u16,
}

// ---------------------------------------------------------------------------
// Enumeration
// ---------------------------------------------------------------------------

/// Enumerate all input and output devices on `host`, collecting
/// GUI-relevant metadata for each.
///
/// Returns `(inputs, outputs)`. Devices that fail to report an ID or name are
/// skipped silently.
pub fn enumerate_devices(host: &cpal::Host) -> (Vec<DeviceInfo>, Vec<DeviceInfo>) {
    let inputs = collect_device_infos(host.input_devices(), true);
    let outputs = collect_device_infos(host.output_devices(), false);
    (inputs, outputs)
}

fn collect_device_infos<I>(iter: Result<I, cpal::DevicesError>, is_input: bool) -> Vec<DeviceInfo>
where
    I: Iterator<Item = cpal::Device>,
{
    let devices = match iter {
        Ok(it) => it.collect::<Vec<_>>(),
        Err(e) => {
            tracing::warn!("failed to enumerate devices: {e}");
            return Vec::new();
        }
    };

    let mut infos = Vec::with_capacity(devices.len());
    for device in devices {
        let id = match device.id() {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("device id error: {e}");
                continue;
            }
        };
        let name = match device.description() {
            Ok(desc) => desc.name().to_owned(),
            Err(e) => {
                tracing::warn!("device name error: {e}");
                continue;
            }
        };

        let (supported_sample_rates, buffer_size_range) =
            extract_device_capabilities(&device, is_input);

        infos.push(DeviceInfo {
            name,
            id_string: id.to_string(),
            device,
            supported_sample_rates,
            buffer_size_range,
        });
    }
    infos
}

/// Inspect all supported stream config ranges on `device` and derive:
/// - which standard sample rates fall within at least one F32/I16 range, and
/// - the union of all reported buffer-size ranges.
fn extract_device_capabilities(
    device: &cpal::Device,
    is_input: bool,
) -> (Vec<u32>, Option<(u32, u32)>) {
    let configs: Vec<cpal::SupportedStreamConfigRange> = if is_input {
        match device.supported_input_configs() {
            Ok(it) => it.collect(),
            Err(_) => return (Vec::new(), None),
        }
    } else {
        match device.supported_output_configs() {
            Ok(it) => it.collect(),
            Err(_) => return (Vec::new(), None),
        }
    };

    let mut buf_min: Option<u32> = None;
    let mut buf_max: Option<u32> = None;

    // For each standard rate, track whether it is covered by at least one
    // F32/I16 config range.
    let mut rate_supported = [false; STANDARD_SAMPLE_RATES.len()];

    for cfg in &configs {
        // Only consider formats cpal can convert from.
        let fmt = cfg.sample_format();
        if fmt != SampleFormat::F32 && fmt != SampleFormat::I16 {
            continue;
        }

        let lo = cfg.min_sample_rate();
        let hi = cfg.max_sample_rate();

        for (i, &rate) in STANDARD_SAMPLE_RATES.iter().enumerate() {
            if rate >= lo && rate <= hi {
                rate_supported[i] = true;
            }
        }

        // Accumulate buffer size range union.
        if let cpal::SupportedBufferSize::Range { min, max } = cfg.buffer_size() {
            let min = *min;
            let max = *max;
            buf_min = Some(buf_min.map_or(min, |prev| prev.min(min)));
            buf_max = Some(buf_max.map_or(max, |prev| prev.max(max)));
        }
    }

    let supported_rates = STANDARD_SAMPLE_RATES
        .iter()
        .zip(rate_supported.iter())
        .filter_map(|(&rate, &ok)| if ok { Some(rate) } else { None })
        .collect();

    let buffer_size_range = buf_min.zip(buf_max);

    (supported_rates, buffer_size_range)
}

// ---------------------------------------------------------------------------
// Computation helpers (pure — testable without real devices)
// ---------------------------------------------------------------------------

/// Return the intersection of the two devices' supported standard sample rates,
/// preserving ascending order.
pub fn compute_common_sample_rates(input: &DeviceInfo, output: &DeviceInfo) -> Vec<u32> {
    input
        .supported_sample_rates
        .iter()
        .copied()
        .filter(|r| output.supported_sample_rates.contains(r))
        .collect()
}

/// Return a list of buffer sizes that both devices support, as powers of two
/// within their overlapping range, plus `None` (meaning
/// [`cpal::BufferSize::Default`]) as the first element.
///
/// If either device has `buffer_size_range == None`, we treat it as
/// unconstrained (any size is fine from that device's perspective). When both
/// are `None`, only `[None]` is returned.
pub fn compute_available_buffer_sizes(input: &DeviceInfo, output: &DeviceInfo) -> Vec<Option<u32>> {
    let mut result = vec![None];

    let effective_range: Option<(u32, u32)> =
        match (input.buffer_size_range, output.buffer_size_range) {
            (Some((in_lo, in_hi)), Some((out_lo, out_hi))) => {
                let lo = in_lo.max(out_lo);
                let hi = in_hi.min(out_hi);
                if lo <= hi { Some((lo, hi)) } else { None }
            }
            (Some(r), None) | (None, Some(r)) => Some(r),
            (None, None) => None,
        };

    if let Some((lo, hi)) = effective_range {
        for &size in POW2_BUFFER_SIZES {
            if size >= lo && size <= hi {
                result.push(Some(size));
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Startup resolution
// ---------------------------------------------------------------------------

/// Resolve a complete set of stream parameters at startup.
///
/// Fallback chain for each of input and output:
/// 1. CLI name substring match (case-insensitive)
/// 2. Saved config `DeviceId` string lookup
/// 3. System default device
///
/// Also returns the full device lists so the GUI can populate its dropdowns
/// without a second enumeration pass.
pub fn resolve_initial_config(
    host: &cpal::Host,
    config: &TonismConfig,
    cli_input: Option<&str>,
    cli_output: Option<&str>,
) -> anyhow::Result<(ResolvedDeviceConfig, Vec<DeviceInfo>, Vec<DeviceInfo>)> {
    let (inputs, outputs) = enumerate_devices(host);

    let input_device = resolve_device(
        host,
        &inputs,
        cli_input,
        config.input_device_id.as_deref(),
        true,
    )
    .context("resolving input device")?;

    let output_device = resolve_device(
        host,
        &outputs,
        cli_output,
        config.output_device_id.as_deref(),
        false,
    )
    .context("resolving output device")?;

    // Pick sample rate: prefer config value if it appears in both devices,
    // otherwise fall back to the first common rate, then 48000.
    let common_rates = {
        // Find the DeviceInfo entries that match the resolved devices so we
        // can call compute_common_sample_rates.
        let in_info = device_info_for(&inputs, &input_device);
        let out_info = device_info_for(&outputs, &output_device);
        match (in_info, out_info) {
            (Some(i), Some(o)) => compute_common_sample_rates(i, o),
            _ => Vec::new(),
        }
    };

    let sample_rate = pick_sample_rate(config.sample_rate, &common_rates);

    // Pick buffer size: prefer config value if within range, else None (Default).
    let in_info = device_info_for(&inputs, &input_device);
    let out_info = device_info_for(&outputs, &output_device);
    let buffer_size = match (in_info, out_info) {
        (Some(i), Some(o)) => {
            let available = compute_available_buffer_sizes(i, o);
            pick_buffer_size(config.buffer_size, &available)
        }
        _ => config.buffer_size,
    };

    // Derive channel count from the output device's default config.
    let channels = output_device
        .default_output_config()
        .map(|c| c.channels())
        .unwrap_or(2);

    let resolved = ResolvedDeviceConfig {
        input_device,
        output_device,
        sample_rate,
        buffer_size,
        channels,
    };

    Ok((resolved, inputs, outputs))
}

/// Find the `&DeviceInfo` in `list` whose device has the same ID as `target`.
fn device_info_for<'a>(list: &'a [DeviceInfo], target: &cpal::Device) -> Option<&'a DeviceInfo> {
    let target_id = target.id().ok()?.to_string();
    list.iter().find(|d| d.id_string == target_id)
}

/// Resolve a single device following the CLI > config > default fallback chain.
fn resolve_device(
    host: &cpal::Host,
    list: &[DeviceInfo],
    cli_name: Option<&str>,
    config_id: Option<&str>,
    is_input: bool,
) -> anyhow::Result<cpal::Device> {
    let direction = if is_input { "input" } else { "output" };

    // 1. CLI substring match.
    if let Some(name) = cli_name {
        let name_lower = name.to_lowercase();
        let found = list
            .iter()
            .find(|d| d.name.to_lowercase().contains(&name_lower));
        match found {
            Some(info) => {
                tracing::info!("CLI: using {direction} device \"{}\"", info.name);
                return Ok(info.device.clone());
            }
            None => {
                eprintln!("No {direction} device matching \"{name}\". Available:");
                for d in list {
                    eprintln!("  - {}", d.name);
                }
                anyhow::bail!("no {direction} device matching \"{name}\"");
            }
        }
    }

    // 2. Saved-config DeviceId lookup.
    if let Some(id_str) = config_id
        && let Ok(device_id) = cpal::DeviceId::from_str(id_str)
    {
        if let Some(dev) = host.device_by_id(&device_id) {
            tracing::info!("Config: using {direction} device \"{}\"", id_str);
            return Ok(dev);
        }
        tracing::warn!("Config {direction} device \"{id_str}\" not found, falling back to default");
    }

    // 3. System default.
    let dev = if is_input {
        host.default_input_device()
            .context("no default input device available")?
    } else {
        host.default_output_device()
            .context("no default output device available")?
    };
    tracing::info!(
        "Default: using {direction} device \"{}\"",
        dev.description()
            .map(|d| d.name().to_owned())
            .unwrap_or_else(|_| "<unknown>".to_owned())
    );
    Ok(dev)
}

/// Choose a sample rate: prefer the config value if it is in `common_rates`,
/// otherwise take the first common rate, otherwise 48000.
fn pick_sample_rate(config_rate: Option<u32>, common_rates: &[u32]) -> u32 {
    if let Some(r) = config_rate
        && common_rates.contains(&r)
    {
        return r;
    }
    common_rates.first().copied().unwrap_or(48000)
}

/// Choose a buffer size: prefer the config value if it appears in `available`
/// (the list produced by [`compute_available_buffer_sizes`], which always
/// starts with `None`), otherwise `None` (Default).
fn pick_buffer_size(config_buf: Option<u32>, available: &[Option<u32>]) -> Option<u32> {
    if let Some(b) = config_buf
        && available.contains(&Some(b))
    {
        return Some(b);
    }
    None // Default
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Stream-level error callback. cpal invokes this off the realtime
/// thread, so plain `eprintln!` is fine.
pub fn err_fn(err: cpal::StreamError) {
    eprintln!("stream error: {err}");
}

/// Best-effort human-readable device label. Mirrors the pattern used
/// by `scripts/check_buffer_size.rs`.
pub fn device_label(device: &cpal::Device) -> String {
    device
        .description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_else(|_| "<unnamed>".into())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_device_info(rates: Vec<u32>, buf_range: Option<(u32, u32)>) -> DeviceInfo {
        // We construct a real cpal device for the struct but only test the
        // pure computation fields — the device field is never accessed.
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .or_else(|| host.default_input_device())
            .expect("at least one audio device must be present for tests");
        DeviceInfo {
            name: "mock".to_owned(),
            id_string: "mock:mock".to_owned(),
            device,
            supported_sample_rates: rates,
            buffer_size_range: buf_range,
        }
    }

    // ------------------------------------------------------------------
    // compute_common_sample_rates
    // ------------------------------------------------------------------

    #[test]
    fn common_rates_intersection() {
        let input = mock_device_info(vec![44100, 48000, 96000], None);
        let output = mock_device_info(vec![48000, 96000, 192000], None);
        assert_eq!(
            compute_common_sample_rates(&input, &output),
            vec![48000, 96000]
        );
    }

    #[test]
    fn common_rates_identical() {
        let d = mock_device_info(vec![44100, 48000], None);
        let d2 = mock_device_info(vec![44100, 48000], None);
        assert_eq!(compute_common_sample_rates(&d, &d2), vec![44100, 48000]);
    }

    #[test]
    fn common_rates_no_overlap_returns_empty() {
        let input = mock_device_info(vec![44100], None);
        let output = mock_device_info(vec![48000], None);
        assert!(compute_common_sample_rates(&input, &output).is_empty());
    }

    // ------------------------------------------------------------------
    // compute_available_buffer_sizes
    // ------------------------------------------------------------------

    #[test]
    fn buffer_sizes_with_range() {
        // Both devices cover 64..=1024
        let input = mock_device_info(vec![], Some((64, 1024)));
        let output = mock_device_info(vec![], Some((64, 1024)));
        let result = compute_available_buffer_sizes(&input, &output);
        // First element must be None (Default)
        assert_eq!(result[0], None);
        // Should contain 64, 128, 256, 512, 1024 (all powers of two in [64,1024])
        assert!(result.contains(&Some(64)));
        assert!(result.contains(&Some(128)));
        assert!(result.contains(&Some(256)));
        assert!(result.contains(&Some(512)));
        assert!(result.contains(&Some(1024)));
        // 32 is below range, 2048 is above range
        assert!(!result.contains(&Some(32)));
        assert!(!result.contains(&Some(2048)));
    }

    #[test]
    fn buffer_sizes_intersection_of_ranges() {
        let input = mock_device_info(vec![], Some((32, 512)));
        let output = mock_device_info(vec![], Some((128, 1024)));
        // Overlap is 128..=512
        let result = compute_available_buffer_sizes(&input, &output);
        assert_eq!(result[0], None);
        assert!(result.contains(&Some(128)));
        assert!(result.contains(&Some(256)));
        assert!(result.contains(&Some(512)));
        assert!(!result.contains(&Some(64)));
        assert!(!result.contains(&Some(1024)));
    }

    #[test]
    fn buffer_sizes_no_overlap_returns_only_default() {
        // Input max < output min → no overlap
        let input = mock_device_info(vec![], Some((32, 64)));
        let output = mock_device_info(vec![], Some((512, 2048)));
        let result = compute_available_buffer_sizes(&input, &output);
        assert_eq!(result, vec![None]);
    }

    #[test]
    fn buffer_sizes_both_none_returns_only_default() {
        let input = mock_device_info(vec![], None);
        let output = mock_device_info(vec![], None);
        assert_eq!(compute_available_buffer_sizes(&input, &output), vec![None]);
    }

    #[test]
    fn buffer_sizes_one_none_uses_other_range() {
        // Output has no reported range — treat as unconstrained, so use input range
        let input = mock_device_info(vec![], Some((128, 512)));
        let output = mock_device_info(vec![], None);
        let result = compute_available_buffer_sizes(&input, &output);
        assert_eq!(result[0], None);
        assert!(result.contains(&Some(128)));
        assert!(result.contains(&Some(256)));
        assert!(result.contains(&Some(512)));
    }
}
