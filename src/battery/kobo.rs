use std::fs::{File, read_dir, read_to_string};
use std::io::{Read, Seek, SeekFrom};
use super::{Battery, Status};
use anyhow::{Error, format_err};

const BATTERY_INTERFACE: &str = "/sys/class/power_supply";

const BATTERY_CAPACITY: &str = "capacity";
const BATTERY_STATUS: &str = "status";

// TODO: health, technology, time_to_full_now, time_to_empty_now
pub struct KoboBattery {
    capacity: File,
    status: File,
}

impl KoboBattery {
    pub fn new() -> Result<KoboBattery, Error> {
        let base = read_dir(BATTERY_INTERFACE)?
            .filter_map(Result::ok)
            .map(|dir| dir.path())
            .find(|path| {
                let kind = read_to_string(path.join("type")).unwrap_or_default();
                kind.trim_end() == "Battery"
            }).expect("Could not find battery");
        let capacity = File::open(base.join(BATTERY_CAPACITY))?;
        let status = File::open(base.join(BATTERY_STATUS))?;
        Ok(KoboBattery { capacity, status })
    }
}

impl Battery for KoboBattery {
    fn capacity(&mut self) -> Result<f32, Error> {
        let mut buf = String::new();
        self.capacity.seek(SeekFrom::Start(0))?;
        self.capacity.read_to_string(&mut buf)?;
        Ok(buf.trim_end().parse::<f32>().unwrap_or(0.0))
    }

    fn status(&mut self) -> Result<Status, Error> {
        let mut buf = String::new();
        self.status.seek(SeekFrom::Start(0))?;
        self.status.read_to_string(&mut buf)?;
        match buf.trim_end() {
            "Discharging" => Ok(Status::Discharging),
            "Charging" => Ok(Status::Charging),
            "Not charging" | "Full" => Ok(Status::Charged),
            _ => Err(format_err!("unknown battery status")),

        }
    }
}
