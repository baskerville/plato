use std::path::PathBuf;

pub const SETTINGS_PATH: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub library_path: PathBuf,
    pub refresh_every: Option<u8>,
    pub summary_size: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            library_path: PathBuf::from("/mnt/onboard/books"),
            refresh_every: Some(24),
            summary_size: 1,
        }
    }
}
