use std::path::PathBuf;
use fnv::FnvHashSet;
use frontlight::LightLevels;

pub const SETTINGS_PATH: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub library_path: PathBuf,
    pub refresh_every: Option<u8>,
    pub summary_size: u8,
    pub import: ImportSettings,
    pub frontlight_levels: LightLevels,
    pub frontlight: bool,
    pub wifi: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportSettings {
    pub unmount_trigger: bool,
    pub allowed_kinds: FnvHashSet<String>,
}

impl Default for ImportSettings {
    fn default() -> Self {
        ImportSettings {
            unmount_trigger: true,
            allowed_kinds: ["pdf", "djvu", "epub", "html",
                            "cbz", "png", "jpg", "jpeg"].iter().map(|k| k.to_string()).collect(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            library_path: PathBuf::from("/mnt/onboard"),
            refresh_every: Some(24),
            summary_size: 1,
            import: ImportSettings::default(),
            frontlight_levels: LightLevels::Standard(0.0),
            frontlight: true,
            wifi: false,
        }
    }
}
