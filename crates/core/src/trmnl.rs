use crate::settings;
use anyhow::{anyhow, Result};
use image::DynamicImage;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

lazy_static! {
    static ref GLOBAL_TRMNL_CLIENT: Arc<Mutex<Option<TrmnlClient>>> = Arc::new(Mutex::new(None));
}

#[derive(Debug, Deserialize)]
struct SetupResponse {
    status: u32,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DisplayResponse {
    status: u32,
    image_url: Option<String>,
    refresh_rate: Option<u32>,
}

#[derive(Clone)]
pub struct TrmnlClient {
    refresh_rate: u32,
    next_refresh_time: Option<Instant>,
}

impl TrmnlClient {
    pub fn new() -> Self {
        Self {
            refresh_rate: 1800,
            next_refresh_time: None,
        }
    }

    pub fn refresh_rate(&self) -> u32 {
        self.refresh_rate
    }

    fn set_next_refresh_time(&mut self) {
        self.next_refresh_time =
            Some(Instant::now() + Duration::from_secs(self.refresh_rate as u64));
    }

    fn setup(&mut self, config: &mut settings::Trmnl) -> Result<()> {
        if config.access_key.is_some() {
            return Ok(());
        }

        let setup_response: SetupResponse = ureq::get(&format!("{}/setup", config.api_base))
            .header("ID", &config.mac_address)
            .call()?
            .body_mut()
            .read_json()?;

        if setup_response.status != 200 {
            return Err(anyhow!(
                "Setup failed, API provided status: {}",
                setup_response.status
            ));
        }

        let api_key = setup_response
            .api_key
            .ok_or_else(|| anyhow!("Missing API key in response"))?;

        config.access_key = Some(api_key);

        Ok(())
    }

    fn fetch_display(&mut self, config: &mut settings::Trmnl) -> Result<DynamicImage> {
        self.setup(config)?;

        let api_key = config
            .access_key
            .as_ref()
            .ok_or_else(|| anyhow!("TRMNL API key not available"))?;

        let display_response: DisplayResponse = ureq::get(&format!("{}/display", config.api_base))
            .header("ID", &config.mac_address)
            .header("Access-Token", api_key)
            .header("Refresh-Rate", "1800")
            .header("Battery-Voltage", "4.0")
            .header("FW-Version", "Plato")
            .header("RSSI", "-60")
            .call()?
            .body_mut()
            .read_json()?;

        if display_response.status != 0 {
            return Err(anyhow!(
                "Display fetch failed, API provided status: {}",
                display_response.status
            ));
        }

        let image_url = display_response
            .image_url
            .ok_or_else(|| anyhow!("Missing image URL in response"))?;

        if let Some(rate) = display_response.refresh_rate {
            self.refresh_rate = rate;
        }

        self.set_next_refresh_time();

        let image_data = ureq::get(&image_url).call()?.body_mut().read_to_vec()?;

        let image = image::load_from_memory(&image_data)?;

        Ok(image)
    }

    pub fn save_current_display(
        &mut self,
        rotation: i8,
        config: &mut settings::Trmnl,
    ) -> Option<PathBuf> {
        let mut image = match self.fetch_display(config) {
            Ok(img) => img,
            Err(e) => {
                eprintln!("Failed to fetch TRMNL display: {:?}", e);
                return None;
            }
        };

        // Always rotate back to landscape upright
        match rotation {
            3 => image = image.rotate90(),
            2 => image = image.rotate180(),
            1 => image = image.rotate270(),
            _ => {}
        }

        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("trmnl_display.png");

        match image.save(&path) {
            Ok(_) => {
                println!(
                    "Saved TRMNL display to: {:?} with given rotation {}",
                    path, rotation
                );
                Some(path)
            }
            Err(e) => {
                eprintln!("Failed to save TRMNL display: {:?}", e);
                None
            }
        }
    }
}
