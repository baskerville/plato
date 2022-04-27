#[macro_use] mod geom;
mod color;
mod device;
mod input;
mod unit;
mod framebuffer;
mod helpers;
mod font;
mod document;
mod library;
mod metadata;
mod settings;
mod frontlight;
mod lightsensor;
mod translate;

use std::env;
use std::path::Path;
use getopts::Options;
use chrono::{Local, TimeZone};
use anyhow::{Error, Context, format_err};
use crate::helpers::datetime_format;
use crate::library::Library;
use crate::settings::{LibraryMode, ImportSettings};
use crate::metadata::{extract_metadata_from_document, extract_metadata_from_filename};
use crate::metadata::{consolidate, rename_from_info};

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut opts = Options::new();

    opts.optflag("h", "help", "Print this help message.");
    opts.optflag("I", "import", "Import new files or update existing files.");
    opts.optflag("C", "clean-up", "Remove reading states with unknown fingerprints.");
    opts.optflag("E", "extract-metadata-document", "Extract metadata from documents.");
    opts.optflag("F", "extract-metadata-filename", "Extract metadata from filenames.");
    opts.optflag("S", "consolidate", "Autocorrect simple typographic mistakes.");
    opts.optflag("N", "rename-from-info", "Rename files based on their information.");
    opts.optopt("k", "allowed-kinds", "Comma separated list of allowed kinds.", "ALLOWED_KINDS");
    opts.optopt("e", "metadata-kinds", "Comma separated list of metadata kinds.", "METADATA_KINDS");
    opts.optopt("a", "added-after", "Only process entries added after the given date-time.", "ADDED_DATETIME");
    opts.optopt("m", "library-mode", "The library mode (`database` or `filesystem`).", "LIBRARY_MODE");

    let matches = opts.parse(&args).context("failed to parse the command line arguments")?;

    if matches.opt_present("h") {
        println!("{}", opts.usage("Usage: plato-import -h|-I|-C|-EFSN [-k ALLOWED_KINDS] [-e METADATA_KINDS] [-a ADDED_DATETIME] [-m LIBRARY_MODE] LIBRARY_PATH"));
        return Ok(());
    }

    if matches.free.is_empty() {
        return Err(format_err!("missing required argument: library path"));
    }

    let library_path = Path::new(&matches.free[0]);

    let mut import_settings = ImportSettings {
        metadata_kinds: ["epub"].iter().map(|k| k.to_string()).collect(),
        .. Default::default()
    };

    if let Some(allowed_kinds) = matches.opt_str("k").map(|v| v.split(',').map(|k| k.to_string()).collect()) {
        import_settings.allowed_kinds = allowed_kinds;
    }

    if let Some(metadata_kinds) = matches.opt_str("e").map(|v| v.split(',').map(|k| k.to_string()).collect()) {
        import_settings.metadata_kinds = metadata_kinds;
    }

    let added_after = matches.opt_str("a").as_ref()
                             .and_then(|v| Local.datetime_from_str(v, datetime_format::FORMAT).ok());

    let mode = matches.opt_str("m").as_ref()
                      .and_then(|v| {
                          match v.as_ref() {
                              "database" => Some(LibraryMode::Database),
                              "filesystem" => Some(LibraryMode::Filesystem),
                              _ => None,
                          }
                      }).unwrap_or(LibraryMode::Database);

    let mut library = Library::new(&library_path, mode);

    if matches.opt_present("I") {
        library.import(&import_settings);
    } else if matches.opt_present("C") {
        library.clean_up();
    } else {
        let opt_extract_metadata_document = matches.opt_present("E");
        let opt_extract_metadata_filename = matches.opt_present("F");
        let opt_consolidate = matches.opt_present("S");
        let opt_rename_from_info = matches.opt_present("N");

        library.apply(|path, info| {
            if added_after.map_or(true, |added| info.added >= added) {
                if opt_extract_metadata_document &&
                   import_settings.metadata_kinds.contains(&info.file.kind) {
                    extract_metadata_from_document(path, info);
                }

                if opt_extract_metadata_filename {
                    extract_metadata_from_filename(path, info);
                }

                if opt_consolidate {
                    consolidate(path, info);
                }

                if opt_rename_from_info {
                    rename_from_info(path, info);
                }
            }
        });
    }

    library.flush();

    Ok(())
}
