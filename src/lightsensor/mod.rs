mod kobo;

use errors::*;

pub use self::kobo::KoboLightSensor;

pub trait LightSensor {
    fn level(&mut self) -> Result<u16>;
}

impl LightSensor for u16 {
    fn level(&mut self) -> Result<u16> {
        Ok(*self)
    }
}
