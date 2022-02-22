mod kobo;
mod fake;

use anyhow::Error;

pub use self::kobo::KoboBattery;
pub use self::fake::FakeBattery;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Status {
    Discharging,
    Charging,
    Charged,
    Unknown
    // Full,
}

pub trait Battery {
    fn capacity(&mut self) -> Result<Vec<f32>, Error>;
    fn status(&mut self) -> Result<Vec<Status>, Error>;
}
