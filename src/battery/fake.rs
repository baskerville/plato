use super::{Battery, Status};
use failure::Error;

pub struct FakeBattery {
    capacity: f32,
    status: Status,
}

impl FakeBattery {
    pub fn new() -> FakeBattery {
        FakeBattery { capacity: 50.0, status: Status::Discharging }
    }
}

impl Battery for FakeBattery {
    fn capacity(&mut self) -> Result<f32, Error> {
        Ok(self.capacity)
    }

    fn status(&mut self) -> Result<Status, Error> {
        Ok(self.status)
    }
}
