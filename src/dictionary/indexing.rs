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
use caseless::default_case_fold_str;

/// The index is partially loaded if `state` isn't `None`.
pub struct Index<R: BufRead> {
    pub entries: Vec<Entry>,
    pub state: Option<R>,
    pub settings: Settings,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub headword: String,
    pub offset: u64,
    pub size: u64,
    pub original: Option<String>,
}

#[derive(Debug, Clone)]
// Settings correspond to options detailed in `dictfmt`
pub struct Settings {
    pub all_characters: bool,
    pub case_sensitive: bool,

}

pub trait IndexReader {
    fn load_and_find(&mut self, headword: &str, fuzzy: bool) -> Vec<Entry>;
    fn find(&self, headword: &str, fuzzy: bool) -> Vec<Entry>;
    fn settings(&self) -> Settings;
}

impl<R: BufRead> IndexReader for Index<R> {
    fn load_and_find(&mut self, headword: &str, fuzzy: bool) -> Vec<Entry> {
        if let Some(br) = self.state.take() {
            if let Ok(mut index) = parse_index_with_settings(br, false, Option::Some(&self.settings)) {
                self.entries.append(&mut index.entries);
            }
        }
        self.find(headword, fuzzy)
    }

    fn find(&self, headword: &str, fuzzy: bool) -> Vec<Entry> {
        find(self.entries.as_ref(), headword, fuzzy)
    }

    fn settings(&self) -> Settings {
        self.settings.clone()
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
    let headword = split.next().ok_or(MissingColumnInIndex(line_number))?;

    // Second column: offset into file.
    let offset = split.next().ok_or(MissingColumnInIndex(line_number))?;
    let offset = decode_number(offset)?;

    // Third column: entry size.
    let size = split.next().ok_or(MissingColumnInIndex(line_number))?;
    let size = decode_number(size)?;

    // Fourth column: optional original headword.
    let original = split.next();

    Ok((headword, offset, size, original))
}

/// Parse the index for a dictionary from a given BufRead compatible object.
/// When `lazy` is `true`, the loop stops once all the metadata entries are parsed.
pub fn parse_index<B: BufRead>(br: B, lazy: bool) -> Result<Index<B>, DictError> {
    parse_index_with_settings(br, lazy, None)
}

// parse_index_with_settings accounts for the following possibilities:
// - lazy parse -> parse index metadata (00-database-*)
// - full parse -> parse whole index
// - resume parse -> resume from lazy parse
fn parse_index_with_settings<B: BufRead>(mut br: B, lazy: bool, settings: Option<&Settings>) -> Result<Index<B>, DictError> {
    let mut found_metadata = false;
    let mut settings_created = false;
    let mut entries = Vec::new();
    let mut line_number = 0;
    let mut line = String::new();

    let mut s = Settings{all_characters: false, case_sensitive: false};

    if let Some(settings) = settings {
        s = settings.clone();
        found_metadata = true;
        settings_created = true;
    }

    while let Ok(nb) = br.read_line(&mut line) {
        if nb == 0 {
            break;
        }
        let (headword, offset, size, original) = parse_line(line.trim_end(), line_number)?;

        if !found_metadata && (headword.starts_with("00-database-") || headword.starts_with("00database")) {
            found_metadata = true;
        } else if found_metadata && !settings_created && !headword.starts_with("00-database-") && !headword.starts_with("00database") {

            // A DICT index may not be case-sensitive, but the indexed headwords may not have been casefolded
            // Therefore if the index is not case-sensitive, we will have to casefold all headwords ourselves along with the query
            let all_chars = !find(entries.as_ref(), "00-database-allchars", false).is_empty();

            let word = if all_chars {
                "00-database-case-sensitive"
            } else {
                "00databasecasesensitive"
            };

            let case_sensitive = !find(entries.as_ref(),word, false).is_empty();
            s.all_characters = all_chars;
            s.case_sensitive = case_sensitive;

            settings_created = true;

            // It is possible for headwords to precede the 00-database- entries so we need to go back and clean them up
            for mut entry in entries.iter_mut() {
                let formatted_entry = &mut Entry{
                    headword: default_case_fold_str(&entry.headword),
                    offset: entry.offset,
                    size: entry.size,
                    original: entry.original.clone()
                };

                entry = formatted_entry;
            }
        }

        let formatted_word: String;

        if !s.case_sensitive {
            formatted_word = default_case_fold_str(headword.as_ref());
        } else {
            formatted_word = headword.to_string();
        }

        entries.push(Entry {
            headword: formatted_word,
            offset,
            size,
            original: original.map(String::from),
        });
        line_number += 1;
        line.clear();

        // Break *after* current headword is committed for lazy load
        if lazy && settings_created {
            break;
        }
    }

    let state = if lazy {
        Some(br)
    } else {
        None
    };

    Ok(Index{entries, state, settings: s})
}

/// Parse the index for a dictionary from a given path.
pub fn parse_index_from_file(path: impl AsRef<Path>, lazy: bool) -> Result<Index<BufReader<File>>, DictError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    parse_index(reader, lazy)
}

fn find(entries: &Vec<Entry>, headword: &str, fuzzy: bool) -> Vec<Entry> {
    if fuzzy {
        entries.iter().filter(|entry| levenshtein(headword, &entry.headword) <= 1).cloned().collect()
    } else {
        if let Ok(mut i) = entries.binary_search_by_key(&headword, |entry| &entry.headword) {
            let mut results = vec![entries[i].clone()];
            let j = i;
            while i > 0 {
                i -= 1;
                if entries[i].headword != headword {
                    break;
                }
                results.insert(0, entries[i].clone());
            }
            i = j;
            while i < entries.len() - 1 {
                i += 1;
                if entries[i].headword != headword {
                    break;
                }
                results.push(entries[i].clone());
            }
            results
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Empty;

    const PATH_CASE_SENSITIVE_INDEX: &str = "src/dictionary/testdata/case_sensitive_dict.index";
    const PATH_CASE_INSENSITIVE_INDEX: &str = "src/dictionary/testdata/case_insensitive_dict.index";

    #[test]
    fn test_index_find() {
        let words = vec![
            Entry{
                headword: String::from("bar"),
                offset: 0,
                size: 8,
                original: None,
            },
            Entry{
                headword: String::from("baz"),
                offset: 8,
                size: 4,
                original: None,
            },
            Entry{
                headword: String::from("foo"),
                offset: 12,
                size: 4,
                original: None,
            },
        ];

        let index: Index<Empty> = Index{
            entries: words,
            state: None,
            settings: Settings{ all_characters: false, case_sensitive: false },
        };

        let r = index.find("apples", false);
        assert!(r.is_empty());

        let r = index.find("baz", false);
        assert!(!r.is_empty());
        assert_eq!(r.len(), 1);
        assert_eq!(r.first().unwrap().headword, "baz");

        let r = index.find("bas", true);
        assert!(!r.is_empty());
        assert_eq!(r.len(), 2);
        assert_eq!(r.first().unwrap().headword, "bar");
    }

    #[test]
    // Make sure that a lazy load does not inadvertently skip a word when it returns to BufRead
    fn test_index_load_and_find() {
        let r = parse_index_from_file(PATH_CASE_INSENSITIVE_INDEX, true);
        assert!(r.is_ok());

        let mut index = r.unwrap();
        assert_eq!(index.entries[0].headword, "00-database-allchars");
        assert_eq!(index.entries.last().unwrap().headword, "bar");

        let r = index.load_and_find("bar", false);
        assert!(!r.is_empty());

        let r = index.load_and_find("foo", false);
        assert!(!r.is_empty());
    }

    #[test]
    fn test_parse_index_from_file() {
        let r = parse_index_from_file(PATH_CASE_INSENSITIVE_INDEX, false);
        assert!(r.is_ok());

        let index = r.unwrap();
        assert_eq!(index.entries[0].headword, "00-database-allchars");
        assert_eq!(index.entries.last().unwrap().headword, "あいおい");
    }

    #[test]
    fn test_parse_index_from_file_lazy() {
        let r = parse_index_from_file(PATH_CASE_INSENSITIVE_INDEX, true);
        assert!(r.is_ok());

        let index = r.unwrap();
        assert_eq!(index.entries[0].headword, "00-database-allchars");
        assert_eq!(index.entries.last().unwrap().headword, "bar");
    }

    #[test]
    fn test_parse_index_from_file_handles_case_insensitivity() {
        let r = parse_index_from_file(PATH_CASE_INSENSITIVE_INDEX, false);
        assert!(r.is_ok());

        let index = r.unwrap();

        let r = index.find("bar", false);
        assert!(!r.is_empty());
        assert_eq!(r.first().unwrap().headword, "bar");

        // straße should fold to strasse
        // https://www.w3.org/International/wiki/Case_folding
        let r = index.find("strasse", false);
        assert!(!r.is_empty());
        assert_eq!(r.first().unwrap().headword, "strasse");

    }

    #[test]
    fn test_parse_index_from_file_handles_case_sensitivity() {
        let r = parse_index_from_file(PATH_CASE_SENSITIVE_INDEX, false);
        assert!(r.is_ok());

        let index = r.unwrap();

        let r = index.find("Bar", false);
        assert!(!r.is_empty());
        assert_eq!(r.first().unwrap().headword, "Bar");

        let r = index.find("straße", false);
        assert!(!r.is_empty());
        assert_eq!(r.first().unwrap().headword, "straße");

    }
}

