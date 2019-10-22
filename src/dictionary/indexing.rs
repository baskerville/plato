//! Parse and decode `*.index` files.
//!
//! Each dictionary file (`*.dict.dz)`) is accompanied by a `*.index` file containing a list of
//! words, together with its (byte) position in the dict file and its (byte) length. This module
//! provides functions to parse this index file.
//!
//! The position and the length of a definition is given in a semi-base64 encoding. It uses all
//! Latin letters (upper and lower case), all digits and additionally, `+` and `/`:
//!
//! `ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/`
//!
//! The calculation works as follows: `sum += x * 64^i`
//!
//! - `i` is the position within the string to calculate the number from and counts from right to
//!   left, starting at 0.
//! - `x` is the index within the array given above, i.e. `'a' == 26`.
//!
//! The sum makes up the index.

use std::fs::File;
use std::path::Path;
use std::io::{BufRead, BufReader};

use levenshtein::levenshtein;

use super::errors::DictError;
use super::errors::DictError::*;

/// The index is partially loaded if `state` isn't `None`.
pub struct Index<R: BufRead> {
    pub words: Vec<Entry>,
    pub state: Option<R>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub word: String,
    pub offset: u64,
    pub size: u64,
    pub original: Option<String>,
}

pub trait IndexReader {
    fn load_and_find(&mut self, word: &str, fuzzy: bool) -> Vec<Entry>;
    fn find(&self, word: &str, fuzzy: bool) -> Vec<Entry>;
}

impl<R: BufRead> IndexReader for Index<R> {
    fn load_and_find(&mut self, word: &str, fuzzy: bool) -> Vec<Entry> {
        if let Some(br) = self.state.take() {
            if let Ok(mut index) = parse_index(br, false) {
                self.words.append(&mut index.words);
            }
        }
        self.find(word, fuzzy)
    }

    fn find(&self, word: &str, fuzzy: bool) -> Vec<Entry> {
        if fuzzy {
            self.words.iter().filter(|entry| levenshtein(word, &entry.word) <= 1).cloned().collect()
        } else {
            if let Ok(i) = self.words.binary_search_by_key(&word, |entry| &entry.word) {
                vec![self.words[i].clone()]
            } else {
                Vec::new()
            }
        }
    }
}

/// Get the assigned number for a character
/// If the character was unknown, an empty Err(()) is returned.
#[inline]
fn get_base(input: char) -> Option<u64> {
    match input {
        'A' ..= 'Z' => Some((input as u64) - 65), // 'A' should become 0
        'a' ..= 'z' => Some((input as u64) - 71), // 'a' should become 26, ...
        '0' ..= '9' => Some((input as u64) + 4), // 0 should become 52
        '+' => Some(62),
        '/' => Some(63),
        _ => None,
    }
}

/// Decode a number from a given String.
///
/// This function decodes a number from the format described in the module documentation. If
/// unknown characters/bytes are encountered, a `DictError` is returned.
pub fn decode_number(word: &str) -> Result<u64, DictError> {
    let mut index = 0u64;
    for (i, character) in word.chars().rev().enumerate() {
        index += match get_base(character) {
            Some(x) => x * 64u64.pow(i as u32),
            None => return Err(InvalidCharacter(character, None, Some(i))),
        };
    }
    Ok(index)
}

/// Parse a single line from the index file.
fn parse_line(line: &str, line_number: usize) -> Result<(&str, u64, u64, Option<&str>), DictError> {
    // First column: headword.
    let mut split = line.split('\t');
    let word = split.next().ok_or(MissingColumnInIndex(line_number))?;

    // Second column: offset into file.
    let offset = split.next().ok_or(MissingColumnInIndex(line_number))?;
    let offset = decode_number(offset)?;

    // Third column: entry size.
    let size = split.next().ok_or(MissingColumnInIndex(line_number))?;
    let size = decode_number(size)?;

    // Fourth column: optional original headword.
    let original = split.next();

    Ok((word, offset, size, original))
}

/// Parse the index for a dictionary from a given BufRead compatible object.
/// When `lazy` is `true`, the loop stops once all the metadata entries are parsed.
pub fn parse_index<B: BufRead>(mut br: B, lazy: bool) -> Result<Index<B>, DictError> {
    let mut info = false;
    let mut words = Vec::new();
    let mut line_number = 0;
    let mut line = String::new();

    while let Ok(nb) = br.read_line(&mut line) {
        if nb == 0 {
            break;
        }
        let (word, offset, size, original) = parse_line(line.trim_end(), line_number)?;
        if lazy {
            if !info && (word.starts_with("00-database-") || word.starts_with("00database")) {
                info = true;
            } else if info && !word.starts_with("00-database-") && !word.starts_with("00database") {
                break;
            }
        }
        words.push(Entry {
            word: word.to_string(),
            offset,
            size,
            original: original.map(String::from),
        });
        line_number += 1;
        line.clear();
    }

    let state = if lazy {
        Some(br)
    } else {
        None
    };

    Ok(Index { words, state })
}

/// Parse the index for a dictionary from a given path.
pub fn parse_index_from_file<P: AsRef<Path>>(path: P, lazy: bool) -> Result<Index<BufReader<File>>, DictError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    parse_index(reader, lazy)
}
