//! A dict format (`*.dict`) reader crate.
//!
//! This crate can read dictionaries in the dict format, as used by dictd. It supports both
//! uncompressed and compressed dictionaries.

mod dictreader;
mod errors;
mod indexing;

use std::path::Path;

use self::dictreader::DictReader;
use self::indexing::IndexReader;

/// A dictionary wrapper.
///
/// A dictionary is made up of a `*.dict` or `*.dict.dz` file with the actual content and a
/// `*.index` file with a list of all headwords and with positions in the dict file + length
/// information. It provides a convenience function to look up headwords directly, without caring
/// about the details of the index and the underlying dict format.
pub struct Dictionary {
    content: Box<dyn DictReader>,
    index: Box<dyn IndexReader>,
    metadata: Metadata,
}

/// The special metadata entries that we care about.
///
/// These entries should appear close to the beginning of the index file.
pub struct Metadata {
    pub all_chars: bool,
    pub case_sensitive: bool,
}

impl Dictionary {
    /// Look up a word in a dictionary.
    ///
    /// Words are looked up in the index and then retrieved from the dict file. If no word was
    /// found, the returned vector is empty. Errors result from the parsing of the underlying files.
    pub fn lookup(&mut self, word: &str, fuzzy: bool) -> Result<Vec<[String; 2]>, errors::DictError> {
        let mut query = word.to_string();
        if !self.metadata.case_sensitive {
            query = query.to_lowercase();
        }
        if !self.metadata.all_chars {
            query = query.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
        }
        let entries = self.index.load_and_find(&query, fuzzy, &self.metadata);
        let mut results = Vec::new();
        for entry in entries.into_iter() {
            results.push([entry.original.unwrap_or(entry.headword),
                          self.content.fetch_definition(entry.offset, entry.size)?]);
        }
        Ok(results)
    }

    /// Retreive metadata from the dictionaries.
    ///
    /// The metadata headwords start with `00-database-` or `00database`.
    pub fn metadata(&mut self, name: &str) -> Result<String, errors::DictError> {
        let mut query = format!("00-database-{}", name);
        if !self.metadata.all_chars {
            query = query.replace(|c: char| !c.is_alphanumeric(), "");
        }
        let entries = self.index.find(&query, false);
        let entry = entries.get(0).ok_or_else(|| errors::DictError::WordNotFound(name.into()))?;
        self.content.fetch_definition(entry.offset, entry.size)
            .map(|def| {
                let start = def.find('\n')
                               .filter(|pos| *pos < def.len() - 1)
                               .unwrap_or(0);
                def[start..].trim().to_string()
            })
    }

    /// Get the short name.
    ///
    /// This returns the short name of a dictionary. This corresponds to the
    /// value passed to the `-s` option of `dictfmt`.
    pub fn short_name(&mut self) -> Result<String, errors::DictError> {
        self.metadata("short")
    }

    /// Get the URL.
    ///
    /// This returns the URL of a dictionary. This corresponds to the
    /// value passed to the `-u` option of `dictfmt`.
    pub fn url(&mut self) -> Result<String, errors::DictError> {
        self.metadata("url")
    }
}

/// Load dictionary from given paths
///
/// A dictionary is made of an index and a dictionary (data) file, both are opened from the given
/// input file names. Gzipped files with the suffix `.dz` will be handled automatically.
pub fn load_dictionary_from_file<P: AsRef<Path>>(content_path: P, index_path: P) -> Result<Dictionary, errors::DictError> {
    let content = dictreader::load_dict(content_path)?;
    let index = Box::new(indexing::parse_index_from_file(index_path, true)?);
    Ok(load_dictionary(content, index))
}

/// Load dictionary from given `DictReader` and `Index`.
///
/// A dictionary is made of an index and a dictionary (data). Both are required for look up. This
/// function allows abstraction from the underlying source by only requiring a
/// `dictReader` as trait object. This way, dictionaries from RAM or similar can be
/// implemented.
pub fn load_dictionary(content: Box<dyn DictReader>, index: Box<dyn IndexReader>) -> Dictionary {
    let all_chars = !index.find("00-database-allchars", false).is_empty();
    let word = if all_chars {
        "00-database-case-sensitive"
    } else {
        "00databasecasesensitive"
    };
    let case_sensitive = !index.find(word, false).is_empty();
    Dictionary { content, index, metadata: Metadata { all_chars, case_sensitive } }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH_CASE_SENSITIVE_DICT: &str = "src/dictionary/testdata/case_sensitive_dict.dict";
    const PATH_CASE_SENSITIVE_INDEX: &str = "src/dictionary/testdata/case_sensitive_dict.index";
    const PATH_CASE_INSENSITIVE_DICT: &str = "src/dictionary/testdata/case_insensitive_dict.dict";
    const PATH_CASE_INSENSITIVE_INDEX: &str = "src/dictionary/testdata/case_insensitive_dict.index";

    fn assert_dict_word_exists(mut dict: Dictionary, headword: &str, definition: &str) -> Dictionary {
        let r = dict.lookup(headword, false);
        assert!(r.is_ok());
        let search = r.unwrap();
        assert_eq!(search.len(), 1);
        assert!(search[0][1].contains(definition));

        dict
    }

    #[test]
    fn test_load_dictionary_from_file() {

        let r = load_dictionary_from_file(PATH_CASE_INSENSITIVE_DICT, PATH_CASE_INSENSITIVE_INDEX);
        assert!(r.is_ok());
    }

    #[test]
    fn test_dictionary_lookup_case_insensitive() {

        let r = load_dictionary_from_file(PATH_CASE_INSENSITIVE_DICT, PATH_CASE_INSENSITIVE_INDEX);
        let mut dict = r.unwrap();

        dict = assert_dict_word_exists(dict, "bar", "test for case-sensitivity");
        dict = assert_dict_word_exists(dict, "Bar", "test for case-sensitivity");
        assert_dict_word_exists(dict, "straße", "test for non-latin case-sensitivity");
    }

    #[test]
    fn test_dictionary_lookup_case_insensitive_fuzzy() {

        let r = load_dictionary_from_file(PATH_CASE_INSENSITIVE_DICT, PATH_CASE_INSENSITIVE_INDEX);
        let mut dict = r.unwrap();

        let r = dict.lookup("ba", true);
        assert!(r.is_ok());
        let search = r.unwrap();
        assert_eq!(search.len(), 1);
        assert_eq!(search[0][0], "bar");
        assert!(search[0][1].contains("test for case-sensitivity"));
    }

    #[test]
    fn test_dictionary_lookup_case_sensitive() {

        let r = load_dictionary_from_file(PATH_CASE_SENSITIVE_DICT, PATH_CASE_SENSITIVE_INDEX);
        let mut dict = r.unwrap();

        dict = assert_dict_word_exists(dict, "Bar", "test for case-sensitivity");
        dict = assert_dict_word_exists(dict, "straße", "test for non-latin case-sensitivity");

        let r = dict.lookup("bar", false);
        assert!(r.unwrap().is_empty());

        let r = dict.lookup("strasse", false);
        assert!(r.unwrap().is_empty());
    }

    #[test]
    fn test_dictionary_lookup_case_sensitive_fuzzy() {

        let r = load_dictionary_from_file(PATH_CASE_SENSITIVE_DICT, PATH_CASE_SENSITIVE_INDEX);
        let mut dict = r.unwrap();

        let r = dict.lookup("Ba", true);
        assert!(r.is_ok());
        let search = r.unwrap();
        assert_eq!(search.len(), 1);
        assert_eq!(search[0][0], "Bar");
        assert!(search[0][1].contains("test for case-sensitivity"));
    }
}

