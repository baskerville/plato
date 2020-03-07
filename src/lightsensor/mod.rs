mod kobo;

use anyhow::Error;

pub use self::kobo::KoboLightSensor;

pub trait LightSensor {
    fn level(&mut self) -> Result<u16, Error>;
}

impl LightSensor for u16 {
    fn level(&mut self) -> Result<u16, Error> {
        Ok(*self)
    }
}
