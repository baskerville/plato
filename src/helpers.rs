extern crate serde_json;
extern crate toml;

use std::path::Path;
use std::fs::{self, File};
use std::cmp::Ordering;
use serde::{Serialize, Deserialize};
use errors::*;

pub fn load_json<T, P: AsRef<Path>>(path: P) -> Result<T> where for<'a> T: Deserialize<'a> {
    let file = File::open(path).chain_err(|| "Can't open file.")?;
    serde_json::from_reader(file).chain_err(|| "Can't parse file.")
}

pub fn save_json<T, P: AsRef<Path>>(data: &T, path: P) -> Result<()> where T: Serialize {
    let file = File::create(path).chain_err(|| "Can't create data file.")?;
    serde_json::to_writer_pretty(file, data).chain_err(|| "Can't serialize data to file.")
}

pub fn load_toml<T, P: AsRef<Path>>(path: P) -> Result<T> where for<'a> T: Deserialize<'a> {
    let s = fs::read_to_string(path).chain_err(|| "Can't read file.")?;
    toml::from_str(&s).chain_err(|| "Can't parse file.")
}

pub fn save_toml<T, P: AsRef<Path>>(data: &T, path: P) -> Result<()> where T: Serialize {
    let s = toml::to_string(data).chain_err(|| "Can't serialize data.")?;
    fs::write(path, &s).chain_err(|| "Can't write to file.")
}

pub fn combine_sort_methods<'a, T, F1, F2>(mut f1: F1, mut f2: F2) -> Box<FnMut(&T, &T) -> Ordering + 'a>
where F1: FnMut(&T, &T) -> Ordering + 'a,
      F2: FnMut(&T, &T) -> Ordering + 'a {
    Box::new(move |x, y| {
        f1(x, y).then_with(|| f2(x, y))
    })
}

pub mod simple_date_format {
    use chrono::{DateTime, Local, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

    pub fn serialize<S>(date: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let s = format!("{}", date.format(FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error> where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Local.datetime_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
    }
}
