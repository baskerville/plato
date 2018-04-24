mod preset;

use std::path::PathBuf;
use fnv::FnvHashSet;
use frontlight::LightLevels;

pub use self::preset::{LightPreset, guess_frontlight};

pub const SETTINGS_PATH: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub library_path: PathBuf,
    pub refresh_every: Option<u8>,
    pub summary_size: u8,
    pub import: ImportSettings,
    pub reader: ReaderSettings,
    pub frontlight_levels: LightLevels,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frontlight_presets: Vec<LightPreset>,
    pub frontlight: bool,
    pub wifi: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportSettings {
    pub unmount_trigger: bool,
    pub allowed_kinds: FnvHashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderSettings {
    pub finished: FinishedAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinishedAction {
    Notify,
    Close,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        ReaderSettings {
            finished: FinishedAction::Notify,
        }
    }
}

impl Default for ImportSettings {
    fn default() -> Self {
        ImportSettings {
            unmount_trigger: true,
            allowed_kinds: ["pdf", "djvu", "epub", "fb2", "html",
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
            reader: ReaderSettings::default(),
            frontlight_levels: LightLevels::default(),
            frontlight_presets: Vec::new(),
            frontlight: true,
            wifi: false,
        }
    }
}
