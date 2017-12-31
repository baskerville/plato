use std::io::{Read, Seek, SeekFrom};
use std::fs::File;
use std::path::Path;
use battery::{Battery, Status};
use errors::*;

const BATTERY_INTERFACE: &str = "/sys/class/power_supply/mc13892_bat";

const BATTERY_CAPACITY: &str = "capacity";
const BATTERY_STATUS: &str = "status";

// TODO: health, technology, time_to_full_now, time_to_empty_now
pub struct KoboBattery {
    capacity: File,
    status: File,
}

impl KoboBattery {
    pub fn new() -> Result<KoboBattery> {
        let base = Path::new(BATTERY_INTERFACE);
        let capacity = File::open(base.join(BATTERY_CAPACITY))?;
        let status = File::open(base.join(BATTERY_STATUS))?;
        Ok(KoboBattery { capacity, status })
    }
}

impl Battery for KoboBattery {
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
