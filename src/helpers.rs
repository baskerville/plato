use std::path::{Path, PathBuf, Component};
use std::fs::{self, File};
use std::cmp::Ordering;
use serde::{Serialize, Deserialize};
use failure::{Error, ResultExt};

pub fn load_json<T, P: AsRef<Path>>(path: P) -> Result<T, Error> where for<'a> T: Deserialize<'a> {
    let file = File::open(path).context("Can't open file.")?;
    serde_json::from_reader(file).context("Can't parse file.").map_err(Into::into)
}

pub fn save_json<T, P: AsRef<Path>>(data: &T, path: P) -> Result<(), Error> where T: Serialize {
    let file = File::create(path).context("Can't create data file.")?;
    serde_json::to_writer_pretty(file, data).context("Can't serialize data to file.").map_err(Into::into)
}

pub fn load_toml<T, P: AsRef<Path>>(path: P) -> Result<T, Error> where for<'a> T: Deserialize<'a> {
    let s = fs::read_to_string(path).context("Can't read file.")?;
    toml::from_str(&s).context("Can't parse file.").map_err(Into::into)
}

pub fn save_toml<T, P: AsRef<Path>>(data: &T, path: P) -> Result<(), Error> where T: Serialize {
    let s = toml::to_string(data).context("Can't serialize data.")?;
    fs::write(path, &s).context("Can't write to file.").map_err(Into::into)
}

pub fn combine_sort_methods<'a, T, F1, F2>(mut f1: F1, mut f2: F2) -> Box<dyn FnMut(&T, &T) -> Ordering + 'a>
where F1: FnMut(&T, &T) -> Ordering + 'a,
      F2: FnMut(&T, &T) -> Ordering + 'a {
    Box::new(move |x, y| {
        f1(x, y).then_with(|| f2(x, y))
    })
}

pub trait Normalize: ToOwned {
    fn normalize(&self) -> Self::Owned;
}

impl Normalize for Path {
    fn normalize(&self) -> PathBuf {
        let mut result = PathBuf::from("");

        for c in self.components() {
            match c {
                Component::ParentDir => { result.pop(); },
                Component::CurDir => (),
                _ => result.push(c),
            }
        }

        result
    }
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
