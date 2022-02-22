use std::fs::File;
use std::path::Path;
use std::io::{Read, Seek, SeekFrom};
use anyhow::Error;
use crate::device::CURRENT_DEVICE;
use super::{Battery, Status};

const BATTERY_INTERFACE_A: &str = "/sys/class/power_supply/mc13892_bat";
const BATTERY_INTERFACE_B: &str = "/sys/class/power_supply/battery";
const POWER_COVER_INTERFACE: &str = "/sys/class/misc/cilix";

const BATTERY_CAPACITY: &str = "capacity";
const BATTERY_STATUS: &str = "status";

const POWER_COVER_CAPACITY: &str = "cilix_bat_capacity";
const POWER_COVER_STATUS: &str = "charge_status";
const POWER_COVER_CONNECTED: &str = "cilix_conn";

pub struct PowerCover {
    capacity: File,
    status: File,
    connected: File,
}

// TODO: health, technology, time_to_full_now, time_to_empty_now
pub struct KoboBattery {
    capacity: File,
    status: File,
    power_cover: Option<PowerCover>,
}

impl KoboBattery {
    pub fn new() -> Result<KoboBattery, Error> {
        let base = if CURRENT_DEVICE.mark() != 8 {
            Path::new(BATTERY_INTERFACE_A)
        } else {
            Path::new(BATTERY_INTERFACE_B)
        };
        let capacity = File::open(base.join(BATTERY_CAPACITY))?;
        let status = File::open(base.join(BATTERY_STATUS))?;
        let power_cover = if CURRENT_DEVICE.has_power_cover() {
            let base = Path::new(POWER_COVER_INTERFACE);
            let capacity = File::open(base.join(POWER_COVER_CAPACITY))?;
            let status = File::open(base.join(POWER_COVER_STATUS))?;
            let connected = File::open(base.join(POWER_COVER_CONNECTED))?;
            Some(PowerCover { capacity, status, connected })
        } else {
            None
        };
        Ok(KoboBattery { capacity, status, power_cover })
    }
}

impl KoboBattery {
    fn is_power_cover_connected(&mut self) -> Result<bool, Error> {
        if let Some(power_cover) = self.power_cover.as_mut() {
            let mut buf = String::new();
            power_cover.connected.seek(SeekFrom::Start(0))?;
            power_cover.connected.read_to_string(&mut buf)?;
            Ok(buf.trim_end().parse::<u8>().map_or(false, |v| v == 1))
        } else {
            Ok(false)
        }
    }
}

impl Battery for KoboBattery {
    fn capacity(&mut self) -> Result<Vec<f32>, Error> {
        let mut buf = String::new();
        self.capacity.seek(SeekFrom::Start(0))?;
        self.capacity.read_to_string(&mut buf)?;
        let capacity = buf.trim_end().parse::<f32>()
                                     .unwrap_or(0.0);
        if matches!(self.is_power_cover_connected(), Ok(true)) {
            let mut buf = String::new();
            self.power_cover.iter_mut().for_each(|power_cover| {
                power_cover.capacity.seek(SeekFrom::Start(0)).ok();
                power_cover.capacity.read_to_string(&mut buf).ok();
            });
            let aux_capacity = buf.trim_end().parse::<f32>()
                                             .unwrap_or(0.0);
            Ok(vec![capacity, aux_capacity])
        } else {
            Ok(vec![capacity])
        }
    }

    fn status(&mut self) -> Result<Vec<Status>, Error> {
        let mut buf = String::new();
        self.status.seek(SeekFrom::Start(0))?;
        self.status.read_to_string(&mut buf)?;
        let status = match buf.trim_end() {
            "Discharging" => Status::Discharging,
            "Charging" => Status::Charging,
            "Not charging" | "Full" => Status::Charged,
            _ => Status::Unknown,

        };
        if matches!(self.is_power_cover_connected(), Ok(true)) {
            let mut buf = String::new();
            self.power_cover.iter_mut().for_each(|power_cover| {
                power_cover.status.seek(SeekFrom::Start(0)).ok();
                power_cover.status.read_to_string(&mut buf).ok();
            });
            let aux_status = match buf.trim_end().parse::<i8>() {
                Ok(0) => Status::Discharging,
                Ok(2) => Status::Charging,
                Ok(3) => Status::Charged,
                _ => Status::Unknown,
            };
            Ok(vec![status, aux_status])
        } else {
            Ok(vec![status])
        }
    }
}
