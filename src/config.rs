use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct TonismConfig {
    pub input_device_id: Option<String>,
    pub output_device_id: Option<String>,
    pub sample_rate: Option<u32>,
    pub buffer_size: Option<u32>,
}

const APP_NAME: &str = "tonism";

pub fn load_config() -> TonismConfig {
    match confy::load::<TonismConfig>(APP_NAME, None) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("[config] failed to load config, using defaults: {e}");
            TonismConfig::default()
        }
    }
}

pub fn save_config(config: &TonismConfig) -> anyhow::Result<()> {
    confy::store(APP_NAME, None, config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_selections() {
        let cfg = TonismConfig::default();
        assert_eq!(cfg.input_device_id, None);
        assert_eq!(cfg.output_device_id, None);
        assert_eq!(cfg.sample_rate, None);
        assert_eq!(cfg.buffer_size, None);
    }

    #[test]
    fn serde_round_trip_via_json() {
        let cfg = TonismConfig {
            input_device_id: Some("CoreAudio:BuiltInMic".into()),
            output_device_id: Some("CoreAudio:BuiltInSpeaker".into()),
            sample_rate: Some(48000),
            buffer_size: Some(512),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: TonismConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, restored);
    }
}
