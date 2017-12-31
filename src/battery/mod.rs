mod kobo;
mod fake;

use errors::*;

pub use self::kobo::KoboBattery;
pub use self::fake::FakeBattery;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Status {
    Discharging,
    Charging,
    Charged,
    // Full,
    // Unknown
}

pub trait Battery {
    fn capacity(&mut self) -> Result<f32>;
    fn status(&mut self) -> Result<Status>;
}
