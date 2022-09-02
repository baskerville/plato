use std::io;
use std::char;
use std::fmt;
use std::str::FromStr;
use std::borrow::Cow;
use std::time::SystemTime;
use std::num::ParseIntError;
use std::fs::{self, File, Metadata};
use std::path::{Path, PathBuf, Component};
use fxhash::FxHashMap;
use std::ops::{Deref, DerefMut};
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::{self, Visitor};
use lazy_static::lazy_static;
use entities::ENTITIES;
use walkdir::DirEntry;
use anyhow::{Error, Context};

lazy_static! {
    pub static ref CHARACTER_ENTITIES: FxHashMap<&'static str, &'static str> = {
        let mut m = FxHashMap::default();
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
    let file = File::open(path.as_ref())
                    .with_context(|| format!("can't open file {}", path.as_ref().display()))?;
    serde_json::from_reader(file)
               .with_context(|| format!("can't parse JSON from {}", path.as_ref().display()))
               .map_err(Into::into)
}

pub fn save_json<T, P: AsRef<Path>>(data: &T, path: P) -> Result<(), Error> where T: Serialize {
    let file = File::create(path.as_ref())
                    .with_context(|| format!("can't create file {}", path.as_ref().display()))?;
    serde_json::to_writer_pretty(file, data)
               .with_context(|| format!("can't serialize to JSON file {}", path.as_ref().display()))
               .map_err(Into::into)
}

pub fn load_toml<T, P: AsRef<Path>>(path: P) -> Result<T, Error> where for<'a> T: Deserialize<'a> {
    let s = fs::read_to_string(path.as_ref())
               .with_context(|| format!("can't read file {}", path.as_ref().display()))?;
    toml::from_str(&s)
         .with_context(|| format!("can't parse TOML content from {}", path.as_ref().display()))
         .map_err(Into::into)
}

pub fn save_toml<T, P: AsRef<Path>>(data: &T, path: P) -> Result<(), Error> where T: Serialize {
    let s = toml::to_string(data)
                 .context("can't convert to TOML format")?;
    fs::write(path.as_ref(), &s)
       .with_context(|| format!("can't write to file {}", path.as_ref().display()))
       .map_err(Into::into)
}

pub trait Fingerprint {
    fn fingerprint(&self, epoch: SystemTime) -> io::Result<Fp>;
}

impl Fingerprint for Metadata {
    fn fingerprint(&self, epoch: SystemTime) -> io::Result<Fp> {
        let m = self.modified()?.duration_since(epoch)
                    .map_or_else(|e| e.duration().as_secs(), |v| v.as_secs());
        Ok(Fp(m.rotate_left(32) ^ self.len()))
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Fp(u64);

impl Deref for Fp {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Fp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromStr for Fp {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        u64::from_str_radix(s, 16).map(Fp)
    }
}

impl fmt::Display for Fp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016X}", self.0)
    }
}

impl Serialize for Fp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

struct FpVisitor;

impl<'de> Visitor<'de> for FpVisitor {
    type Value = Fp;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Self::Value::from_str(value)
             .map_err(|e| E::custom(format!("can't parse fingerprint: {}", e)))
    }
}

impl<'de> Deserialize<'de> for Fp {
    fn deserialize<D>(deserializer: D) -> Result<Fp, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(FpVisitor)
    }
}

pub trait Normalize: ToOwned {
    fn normalize(&self) -> Self::Owned;
}

impl Normalize for Path {
    fn normalize(&self) -> PathBuf {
        let mut result = PathBuf::default();

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

pub mod datetime_format {
    use chrono::{DateTime, Local, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    pub const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

    pub fn serialize<S>(date: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let s = format!("{}", date.format(FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error> where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Local.datetime_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
    }
}

pub trait IsHidden {
    fn is_hidden(&self) -> bool;
}

impl IsHidden for DirEntry {
    fn is_hidden(&self) -> bool {
        self.file_name()
             .to_str()
             .map_or(false, |s| s.starts_with('.'))
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
