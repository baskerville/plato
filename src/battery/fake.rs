use battery::{Battery, Status};
use errors::*;

pub struct FakeBattery {
    capacity: f32,
    status: Status,
}

impl FakeBattery {
    pub fn new() -> FakeBattery {
        FakeBattery { capacity: 100.0, status: Status::Discharging }
    }
}

impl Battery for FakeBattery {
    fn capacity(&mut self) -> Result<f32> {
        Ok(self.capacity)
    }

    fn status(&mut self) -> Result<Status> {
        Ok(self.status)
    }
}
