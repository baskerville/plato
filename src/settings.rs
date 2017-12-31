use std::path::PathBuf;
use frontlight::LightLevels;

pub const SETTINGS_PATH: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub library_path: PathBuf,
    pub refresh_every: Option<u8>,
    pub summary_size: u8,
    pub frontlight_levels: LightLevels,
    pub frontlight: bool,
    pub wifi: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            library_path: PathBuf::from("/mnt/onboard"),
            refresh_every: Some(24),
            summary_size: 1,
            frontlight_levels: LightLevels::Standard(0.0),
            frontlight: true,
            wifi: false,
        }
    }
}
