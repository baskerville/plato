use battery::{Battery, Status};
use errors::*;

pub struct RemarkableBattery {
    capacity: f32,
    status: Status,
}

impl RemarkableBattery {
    pub fn new() -> RemarkableBattery {
        RemarkableBattery { capacity: 50.0, status: Status::Discharging }
    }
}

impl Battery for RemarkableBattery {
    fn capacity(&mut self) -> Result<f32> {
        Ok(self.capacity)
    }

    fn status(&mut self) -> Result<Status> {
        Ok(self.status)
    }
}
