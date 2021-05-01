mod preset;

use std::env;
use std::fmt::{self, Debug};
use std::path::PathBuf;
use std::collections::BTreeMap;
use fxhash::{FxHashMap, FxHashSet};
use serde::{Serialize, Deserialize};
use crate::metadata::{SortMethod, TextAlign};
use crate::frontlight::LightLevels;
use crate::color::BLACK;
use crate::device::CURRENT_DEVICE;
use crate::unit::mm_to_px;

pub use self::preset::{LightPreset, guess_frontlight};

pub const SETTINGS_PATH: &str = "Settings.toml";
pub const DEFAULT_FONT_PATH: &str = "/mnt/onboard/fonts";
pub const INTERNAL_CARD_ROOT: &str = "/mnt/onboard";
pub const EXTERNAL_CARD_ROOT: &str = "/mnt/sd";
// Default font size in points.
pub const DEFAULT_FONT_SIZE: f32 = 11.0;
// Default margin width in millimeters.
pub const DEFAULT_MARGIN_WIDTH: i32 = 8;
// Default line height in ems.
pub const DEFAULT_LINE_HEIGHT: f32 = 1.2;
// Default font family name.
pub const DEFAULT_FONT_FAMILY: &str = "Libertinus Serif";
// Default text alignment.
pub const DEFAULT_TEXT_ALIGN: TextAlign = TextAlign::Left;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RotationLock {
    Landscape,
    Portrait,
    Current,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ButtonScheme {
    Natural,
    Inverted,
}

impl fmt::Display for ButtonScheme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    pub selected_library: usize,
    pub keyboard_layout: String,
    pub frontlight: bool,
    pub wifi: bool,
    pub sleep_cover: bool,
    pub auto_share: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation_lock: Option<RotationLock>,
    pub button_scheme: ButtonScheme,
    pub auto_suspend: u8,
    pub auto_power_off: u8,
    pub time_format: String,
    pub date_format: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<LibrarySettings>,
    #[serde(skip_serializing_if = "FxHashMap::is_empty")]
    pub intermission_images: FxHashMap<String, PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frontlight_presets: Vec<LightPreset>,
    pub home: HomeSettings,
    pub reader: ReaderSettings,
    pub import: ImportSettings,
    pub dictionary: DictionarySettings,
    pub sketch: SketchSettings,
    pub calculator: CalculatorSettings,
    pub battery: BatterySettings,
    pub frontlight_levels: LightLevels,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LibraryMode {
    Database,
    Filesystem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct LibrarySettings {
    pub name: String,
    pub path: PathBuf,
    pub mode: LibraryMode,
    pub sort_method: SortMethod,
    pub first_column: FirstColumn,
    pub second_column: SecondColumn,
    pub thumbnail_previews: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<Hook>,
}

impl Default for LibrarySettings {
    fn default() -> Self {
        LibrarySettings {
            name: "Unnamed".to_string(),
            path: env::current_dir().ok()
                      .unwrap_or_else(|| PathBuf::from("/")),
            mode: LibraryMode::Database,
            sort_method: SortMethod::Opened,
            first_column: FirstColumn::TitleAndAuthor,
            second_column: SecondColumn::Progress,
            thumbnail_previews: true,
            hooks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ImportSettings {
    pub unshare_trigger: bool,
    pub startup_trigger: bool,
    pub extract_epub_metadata: bool,
    pub allowed_kinds: FxHashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DictionarySettings {
    pub margin_width: i32,
    pub font_size: f32,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub languages: BTreeMap<String, Vec<String>>,
}

impl Default for DictionarySettings {
    fn default() -> Self {
        DictionarySettings {
            font_size: 11.0,
            margin_width: 4,
            languages: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SketchSettings {
    pub save_path: PathBuf,
    pub notify_success: bool,
    pub pen: Pen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct CalculatorSettings {
    pub font_size: f32,
    pub margin_width: i32,
    pub history_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Pen {
    pub size: i32,
    pub dynamic: bool,
    pub color: u8,
    pub min_speed: f32,
    pub max_speed: f32,
}

impl Default for Pen {
    fn default() -> Self {
        Pen {
            size: 2,
            color: BLACK,
            dynamic: true,
            min_speed: mm_to_px(3.0, CURRENT_DEVICE.dpi),
            max_speed: mm_to_px(152.4, CURRENT_DEVICE.dpi),
        }
    }
}

impl Default for SketchSettings {
    fn default() -> Self {
        SketchSettings {
            save_path: PathBuf::from("Sketches"),
            notify_success: true,
            pen: Pen::default(),
        }
    }
}

impl Default for CalculatorSettings {
    fn default() -> Self {
        CalculatorSettings {
            font_size: 8.0,
            margin_width: 2,
            history_size: 4096,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Columns {
    first: FirstColumn,
    second: SecondColumn,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FirstColumn {
    TitleAndAuthor,
    FileName,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecondColumn {
    Progress,
    Year,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Hook {
    pub path: PathBuf,
    pub program: PathBuf,
    pub sort_method: Option<SortMethod>,
    pub first_column: Option<FirstColumn>,
    pub second_column: Option<SecondColumn>,
}

impl Default for Hook {
    fn default() -> Self {
        Hook {
            path: PathBuf::default(),
            program: PathBuf::default(),
            sort_method: None,
            first_column: None,
            second_column: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct HomeSettings {
    pub address_bar: bool,
    pub navigation_bar: bool,
    pub max_levels: usize,
    pub max_trash_size: u64,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RefreshRateSettings {
    pub regular: u8,
    pub inverted: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ReaderSettings {
    pub finished: FinishedAction,
    pub south_east_corner: SouthEastCornerAction,
    pub strip_width: f32,
    pub corner_width: f32,
    pub font_path: String,
    pub font_family: String,
    pub font_size: f32,
    pub text_align: TextAlign,
    pub margin_width: i32,
    pub line_height: f32,
    pub dithered_kinds: FxHashSet<String>,
    pub refresh_rate: RefreshRateSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct BatterySettings {
    pub warn: f32,
    pub power_off: f32,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinishedAction {
    Notify,
    Close,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SouthEastCornerAction {
    NextPage,
    GoToPage,
}

impl Default for RefreshRateSettings {
    fn default() -> Self {
        RefreshRateSettings {
            regular: 8,
            inverted: 2,
        }
    }
}

impl Default for HomeSettings {
    fn default() -> Self {
        HomeSettings {
            address_bar: false,
            navigation_bar: true,
            max_levels: 3,
            max_trash_size: 32 * (1 << 20),
        }
    }
}

impl Default for ReaderSettings {
    fn default() -> Self {
        ReaderSettings {
            finished: FinishedAction::Notify,
            south_east_corner: SouthEastCornerAction::GoToPage,
            strip_width: 0.6,
            corner_width: 0.4,
            font_path: DEFAULT_FONT_PATH.to_string(),
            font_family: DEFAULT_FONT_FAMILY.to_string(),
            font_size: DEFAULT_FONT_SIZE,
            text_align: DEFAULT_TEXT_ALIGN,
            margin_width: DEFAULT_MARGIN_WIDTH,
            line_height: DEFAULT_LINE_HEIGHT,
            dithered_kinds: ["cbz", "png", "jpg", "jpeg"].iter().map(|k| k.to_string()).collect(),
            refresh_rate: RefreshRateSettings::default(),
        }
    }
}

impl Default for ImportSettings {
    fn default() -> Self {
        ImportSettings {
            unshare_trigger: true,
            startup_trigger: true,
            extract_epub_metadata: true,
            allowed_kinds: ["pdf", "djvu", "epub", "fb2",
                            "xps", "oxps", "html", "htm",
                            "cbz", "png", "jpg", "jpeg"].iter().map(|k| k.to_string()).collect(),
        }
    }
}

impl Default for BatterySettings {
    fn default() -> Self {
        BatterySettings {
            warn: 10.0,
            power_off: 3.0,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            selected_library: if CURRENT_DEVICE.has_removable_storage() { 1 } else { 0 },
            libraries: vec![
                LibrarySettings {
                    name: "On Board".to_string(),
                    path: PathBuf::from(INTERNAL_CARD_ROOT),
                    hooks: vec![
                        Hook {
                            path: PathBuf::from("Articles"),
                            program: PathBuf::from("bin/article_fetcher/article_fetcher"),
                            sort_method: Some(SortMethod::Added),
                            first_column: Some(FirstColumn::TitleAndAuthor),
                            second_column: Some(SecondColumn::Progress),
                        }
                    ],
                    .. Default::default()
                },
                LibrarySettings {
                    name: "Removable".to_string(),
                    path: PathBuf::from(EXTERNAL_CARD_ROOT),
                    .. Default::default()
                },
            ],
            keyboard_layout: "English".to_string(),
            frontlight: true,
            wifi: false,
            sleep_cover: true,
            auto_share: false,
            rotation_lock: None,
            button_scheme: ButtonScheme::Natural,
            auto_suspend: 30,
            auto_power_off: 3,
            time_format: "%H:%M".to_string(),
            date_format: "%A, %B %-d, %Y".to_string(),
            intermission_images: FxHashMap::default(),
            home: HomeSettings::default(),
            reader: ReaderSettings::default(),
            import: ImportSettings::default(),
            dictionary: DictionarySettings::default(),
            sketch: SketchSettings::default(),
            calculator: CalculatorSettings::default(),
            battery: BatterySettings::default(),
            frontlight_levels: LightLevels::default(),
            frontlight_presets: Vec::new(),
        }
    }
}
