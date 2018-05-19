use std::io::{Read, Seek, SeekFrom};
use std::fs::File;
use std::path::Path;
use battery::{Battery, Status};
use errors::*;

const BATTERY_INTERFACE: &str = "/sys/class/power_supply/bq27441";

const BATTERY_CAPACITY: &str = "capacity";
const BATTERY_STATUS: &str = "status";

// TODO: health, technology, time_to_full_now, time_to_empty_now
pub struct RemarkableBattery {
    capacity: File,
    status: File,
}

impl RemarkableBattery {
    pub fn new() -> Result<RemarkableBattery> {
        let base = Path::new(BATTERY_INTERFACE);
        let capacity = File::open(base.join(BATTERY_CAPACITY))?;
        let status = File::open(base.join(BATTERY_STATUS))?;
        Ok(RemarkableBattery { capacity, status })
    }
}

impl Battery for RemarkableBattery {
    fn capacity(&mut self) -> Result<f32> {
        let mut buf = String::new();
        self.capacity.seek(SeekFrom::Start(0))?;
        self.capacity.read_to_string(&mut buf)?;
        Ok(buf.trim_right().parse::<f32>().unwrap_or(0.0))
    }

    fn status(&mut self) -> Result<Status> {
        let mut buf = String::new();
        self.status.seek(SeekFrom::Start(0))?;
        self.status.read_to_string(&mut buf)?;
        match buf.trim_right() {
            "Discharging" => Ok(Status::Discharging),
            "Charging" => Ok(Status::Charging),
            "Not charging" | "Full" => Ok(Status::Charged),
            _ => Err(Error::from("Unknown battery status.")),

        }
    }
}
