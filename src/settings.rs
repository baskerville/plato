#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    library_path: String,
    refresh_every: Option<u8>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            library_path: "/mnt/onboard/books".to_string(),
            refresh_every: Some(24),
        }
    }
}
