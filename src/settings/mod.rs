mod preset;

use std::path::PathBuf;
use fnv::FnvHashSet;
use frontlight::LightLevels;

pub use self::preset::{LightPreset, guess_frontlight};

pub const SETTINGS_PATH: &str = "Settings.toml";
pub const DEFAULT_FONT_PATH: &str = "/mnt/onboard/fonts";
// Default font size in points
pub const DEFAULT_FONT_SIZE: f32 = 11.0;
// Default margin width in millimeters
pub const DEFAULT_MARGIN_WIDTH: i32 = 8;
// Default line height in ems
pub const DEFAULT_LINE_HEIGHT: f32 = 1.2;
// Default font family name
pub const DEFAULT_FONT_FAMILY: &str = "Libertinus Serif";


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    pub library_path: PathBuf,
    pub frontlight: bool,
    pub wifi: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frontlight_presets: Vec<LightPreset>,
    pub home: HomeSettings,
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
pub struct HomeSettings {
    pub summary_size: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ReaderSettings {
    pub refresh_every: u8,
    pub finished: FinishedAction,
    pub epub_engine: EpubEngine,
    pub font_path: String,
    pub font_family: String,
    pub font_size: f32,
    pub margin_width: i32,
    pub line_height: f32,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EpubEngine {
    BuiltIn,
    Mupdf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinishedAction {
    Notify,
    Close,
}

impl Default for HomeSettings {
    fn default() -> Self {
        HomeSettings {
            summary_size: 1,
        }
    }
}

impl Default for ReaderSettings {
    fn default() -> Self {
        ReaderSettings {
            refresh_every: 0,
            finished: FinishedAction::Notify,
            epub_engine: EpubEngine::BuiltIn,
            font_path: DEFAULT_FONT_PATH.to_string(),
            font_family: DEFAULT_FONT_FAMILY.to_string(),
            font_size: DEFAULT_FONT_SIZE,
            margin_width: DEFAULT_MARGIN_WIDTH,
            line_height: DEFAULT_LINE_HEIGHT,
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
            frontlight: true,
            wifi: false,
            home: HomeSettings::default(),
            reader: ReaderSettings::default(),
            import: ImportSettings::default(),
            frontlight_levels: LightLevels::default(),
            frontlight_presets: Vec::new(),
        }
    }
}
