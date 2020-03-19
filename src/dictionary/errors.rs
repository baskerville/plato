//! Errors for the Dict dictionary crate.
use std::error;

/// Error type, representing the errors which can be returned by the libdict library.
///
/// This enum represents a handful of custom errors and wraps `io:::Error` and
/// `string::FromUtf8Error`.
#[derive(Debug)]
pub enum DictError {
    /// Invalid character, e.g. within the index file; the error contains the erroneous character,
    /// and optionally line and position.
    InvalidCharacter(char, Option<usize>, Option<usize>),
    /// Occurs whenever a line in an index file misses a column.
    MissingColumnInIndex(usize),
    /// Invalid file format, contains an explanation an optional path to the
    /// file with the invalid file format.
    InvalidFileFormat(String, Option<String>),
    /// This reports a malicious / malformed index file, which requests a buffer which is too large.
    MemoryError,
    /// This reports words which are not present in the dictionary.
    WordNotFound(String),
    /// A wrapped io::Error.
    IoError(::std::io::Error),
    /// A wrapped Utf8Error.
    Utf8Error(::std::string::FromUtf8Error),
    /// Errors thrown by the flate2 crate - not really descriptive errors, though.
    DeflateError(flate2::DecompressError),
}

impl ::std::fmt::Display for DictError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            DictError::IoError(ref e) => e.fmt(f),
            DictError::Utf8Error(ref e) => e.fmt(f),
            DictError::DeflateError(ref err) => write!(f, "Error while using \
                        the flate2 crate: {:?}", err),
            DictError::MemoryError => write!(f, "not enough memory available"),
            DictError::WordNotFound(ref word) => write!(f, "Word not found: {}", word),
            DictError::InvalidCharacter(ref ch, ref line, ref pos) => {
                let mut ret = write!(f, "Invalid character {}", ch);
                if let Some(ln) = *line {
                    ret = write!(f, " on line {}", ln);
                }
                if let Some(pos) = *pos {
                    ret = write!(f, " at position {}", pos);
                }
                ret
            },
            DictError::MissingColumnInIndex(ref lnum) => write!(f, "line {}: not \
                    enough <tab>-separated columns found, expected at least 3", lnum),
            DictError::InvalidFileFormat(ref explanation, ref path) =>
                write!(f, "{}{}", path.clone().unwrap_or_else(String::new), explanation)
        }
    }
}

impl error::Error for DictError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            DictError::IoError(ref err) => err.source(),
            DictError::Utf8Error(ref err) => err.source(),
            _ => None,
        }
    }
}

// Allow seamless coercion from::Error.
impl From<::std::io::Error> for DictError {
    fn from(err: ::std::io::Error) -> DictError {
        DictError::IoError(err)
    }
}

impl From<::std::string::FromUtf8Error> for DictError {
    fn from(err: ::std::string::FromUtf8Error) -> DictError {
        DictError::Utf8Error(err)
    }
}

impl From<flate2::DecompressError> for DictError {
    fn from(err: flate2::DecompressError) -> DictError {
        DictError::DeflateError(err)
    }
}

