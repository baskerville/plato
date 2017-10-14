extern crate serde_json;

use std::fs::File;
use std::path::{Path, PathBuf};
use errors::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub library_path: PathBuf,
    pub refresh_every: Option<u8>,
}

impl Settings {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Settings> {
        let file = File::open(path).chain_err(|| "Can't open settings file")?;
        serde_json::from_reader(file).chain_err(|| "Can't parse settings file")
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            library_path: PathBuf::from("/mnt/onboard/books"),
            refresh_every: Some(24),
        }
    }
}
