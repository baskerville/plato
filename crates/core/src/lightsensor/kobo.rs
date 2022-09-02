use std::io::{Read, Seek, SeekFrom};
use std::fs::File;
use anyhow::Error;
use super::LightSensor;

// The Aura ONE uses a Silicon Graphics light sensor,
// the model code is si114x (where x is 5, 6, or 7).
const VISIBLE_PHOTODIODE: &str = "/sys/devices/virtual/input/input3/als_vis_data";

pub struct KoboLightSensor(File);

impl KoboLightSensor {
    pub fn new() -> Result<Self, Error> {
        let file = File::open(VISIBLE_PHOTODIODE)?;
        Ok(KoboLightSensor(file))
    }
}

impl LightSensor for KoboLightSensor {
    fn level(&mut self) -> Result<u16, Error> {
        let mut buf = String::new();
        self.0.seek(SeekFrom::Start(0))?;
        self.0.read_to_string(&mut buf)?;
        let value = buf.trim_end().parse()?;
        Ok(value)
    }
}
