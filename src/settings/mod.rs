mod preset;

use std::path::PathBuf;
use fnv::FnvHashSet;
use frontlight::LightLevels;

pub use self::preset::{LightPreset, guess_frontlight};

pub const SETTINGS_PATH: &str = "Settings.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    pub library_path: PathBuf,
    pub summary_size: u8,
    pub frontlight: bool,
    pub wifi: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frontlight_presets: Vec<LightPreset>,
    pub reader: ReaderSettings,
    pub import: ImportSettings,
    pub frontlight_levels: LightLevels,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ImportSettings {
    pub unmount_trigger: bool,
    pub allowed_kinds: FnvHashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ReaderSettings {
    pub refresh_every: u8,
    pub finished: FinishedAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinishedAction {
    Notify,
    Close,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        ReaderSettings {
            refresh_every: 0,
            finished: FinishedAction::Notify,
        }
    }
}

impl Default for ImportSettings {
    fn default() -> Self {
        ImportSettings {
            unmount_trigger: true,
            allowed_kinds: ["pdf", "djvu", "epub",
                            "fb2", "cbz"].iter().map(|k| k.to_string()).collect(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            library_path: PathBuf::from("/mnt/onboard"),
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
