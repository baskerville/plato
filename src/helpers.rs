use std::char;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::fs::{self, File};
use std::path::{Path, PathBuf, Component};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use lazy_static::lazy_static;
use entities::ENTITIES;
use failure::{Error, ResultExt};

lazy_static! {
    pub static ref CHARACTER_ENTITIES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        for e in ENTITIES.iter() {
            m.insert(e.entity, e.characters);
        }
        m
    };
}

pub fn decode_entities(text: &str) -> Cow<str> {
    if text.find('&').is_none() {
        return Cow::Borrowed(text);
    }

    let mut cursor = text;
    let mut buf = String::with_capacity(text.len());

    while let Some(start_index) = cursor.find('&') {
        buf.push_str(&cursor[..start_index]);
        cursor = &cursor[start_index..];
        if let Some(end_index) = cursor.find(';') {
            if let Some(repl) = CHARACTER_ENTITIES.get(&cursor[..=end_index]) {
                buf.push_str(repl);
            } else if cursor[1..].starts_with('#') {
                let radix = if cursor[2..].starts_with('x') {
                    16
                } else {
                    10
                };
                let drift_index = 2 + radix as usize / 16;
                if let Some(ch) = u32::from_str_radix(&cursor[drift_index..end_index], radix)
                                      .ok().and_then(char::from_u32) {
                    buf.push(ch);
                } else {
                    buf.push_str(&cursor[..=end_index]);
                }
            } else {
                buf.push_str(&cursor[..=end_index]);
            }
            cursor = &cursor[end_index+1..];
        } else {
            break;
        }
    }

    buf.push_str(cursor);
    Cow::Owned(buf)
}

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

pub trait AsciiExtension {
    fn to_alphabetic_digit(self) -> Option<u32>;
}

impl AsciiExtension for char {
    fn to_alphabetic_digit(self) -> Option<u32> {
        if self.is_ascii_uppercase() {
            Some(self as u32 - 65)
        } else {
            None
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entities() {
        assert_eq!(decode_entities("a &amp b"), "a &amp b");
        assert_eq!(decode_entities("a &zZz; b"), "a &zZz; b");
        assert_eq!(decode_entities("a &amp; b"), "a & b");
        assert_eq!(decode_entities("a &#x003E; b"), "a > b");
        assert_eq!(decode_entities("a &#38; b"), "a & b");
        assert_eq!(decode_entities("a &lt; b &gt; c"), "a < b > c");
    }
}
