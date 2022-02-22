use anyhow::Error;
use super::{Battery, Status};

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
    fn capacity(&mut self) -> Result<Vec<f32>, Error> {
        Ok(vec![self.capacity])
    }

    fn status(&mut self) -> Result<Vec<Status>, Error> {
        Ok(vec![self.status])
    }
}
