#[macro_use] mod geom;
mod color;
mod device;
mod input;
mod unit;
mod framebuffer;
mod helpers;
mod font;
mod document;
mod metadata;
mod settings;
mod frontlight;
mod lightsensor;
mod symbolic_path;

use std::env;
use std::fs;
use std::process;
use std::io::Read;
use std::path::Path;
use failure::{Error, ResultExt, format_err};
use regex::Regex;
use getopts::Options;
use titlecase::titlecase;
use crate::helpers::{load_json, save_json};
use crate::settings::{ImportSettings, CategoryProvider, EpubEngine};
use crate::metadata::{Info, Metadata, METADATA_FILENAME, IMPORTED_MD_FILENAME};
use crate::metadata::{import, extract_metadata};
use crate::document::epub::xml::decode_entities;
use crate::document::{DocumentOpener, asciify};

fn run() -> Result<(), Error> {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut opts = Options::new();

    opts.optflag("h", "help", "Print this help message.");
    opts.optflag("I", "import", "Import new books.");
    opts.optflag("S", "extract-isbn", "Try to extract identifiers from books.");
    opts.optflag("R", "retrieve-metadata", "Try to retrieve missing metadata.");
    opts.optflag("s", "strict", "Only use the ISBN when retreiving metadata.");
    opts.optflag("M", "extract-metadata", "Try to extract metadata from ePUBs.");
    opts.optflag("C", "consolidate", "Consolidate an existing database.");
    opts.optflag("N", "rename", "Rename files based on their info.");
    opts.optflag("Y", "synchronize", "Synchronize libraries.");
    opts.optflag("Z", "initialize", "Initialize a database.");
    opts.optflag("t", "traverse-hidden", "Traverse hidden directories.");
    opts.optopt("a", "allowed-kinds", "Comma separated list of allowed kinds.", "ALLOWED_KINDS");
    opts.optopt("c", "category-providers", "Comma separated list of category providers.", "CATEGORY_PROVIDERS");
    opts.optopt("i", "input", "Input file name.", "INPUT_NAME");
    opts.optopt("o", "output", "Output file name.", "OUTPUT_NAME");

    let matches = opts.parse(&args).context("Failed to parse the command line arguments.")?;

    if matches.opt_present("h") {
        println!("{}", opts.usage("Usage: plato-import -h|-I|-S|-R[s]|-M|-C|-N|-Z|-Y [-t] [-a ALLOWED_KINDS] [-c CATEGORY_PROVIDERS] [-i INPUT_NAME] [-o OUTPUT_NAME] LIBRARY_PATH [DEST_LIBRARY_PATH]"));
        return Ok(());
    }

    if matches.free.is_empty() {
        return Err(format_err!("Missing required argument: library path."));
    }

    let library_path = Path::new(&matches.free[0]);
    let input_name = matches.opt_str("i").unwrap_or_else(|| METADATA_FILENAME.to_string());
    let output_name = matches.opt_str("o").unwrap_or_else(|| IMPORTED_MD_FILENAME.to_string());

    let input_path = library_path.join(&input_name);
    let output_path = library_path.join(&output_name);
    let mut import_settings = ImportSettings::default();
    import_settings.traverse_hidden = matches.opt_present("t");
    if let Some(allowed_kinds) = matches.opt_str("a").map(|v| v.split(',').map(|k| k.to_string()).collect()) {
        import_settings.allowed_kinds = allowed_kinds;
    }
    if let Some(category_providers) = matches.opt_str("c").map(|v| v.split(',').filter_map(|k| CategoryProvider::from_str(k)).collect()) {
        import_settings.category_providers = category_providers;
    }

    if matches.opt_present("Z") {
        if input_path.exists() {
            return Err(format_err!("File already exists: {}.", input_path.display()));
        } else {
            save_json::<Metadata, _>(&vec![], input_path)?;
        }
    } else if matches.opt_present("I") {
        let metadata = load_json(input_path)?;
        let metadata = import(library_path, &metadata, &import_settings)?;
        save_json(&metadata, output_path)?;
    } else {
        let mut metadata = load_json(&output_path)?;

        if matches.opt_present("S") {
            extract_isbn(library_path, &mut metadata);
        }

        if matches.opt_present("R") {
            retrieve(&mut metadata, matches.opt_present("s"));
        }

        if matches.opt_present("M") {
            extract_metadata(library_path, &mut metadata, &import_settings);
        }

        if matches.opt_present("C") {
            consolidate(&mut metadata);
        }

        if matches.opt_present("N") {
            rename(library_path, &mut metadata);
        }

        if matches.opt_present("Y") {
            if matches.free.len() < 2 {
                return Err(format_err!("Missing required argument: destination library path."));
            }

            let dest_library_path = Path::new(&matches.free[1]);

            synchronize(library_path, dest_library_path, &metadata);
        }
        
        save_json(&metadata, output_path)?;
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        for e in e.iter_chain() {
            eprintln!("plato-import: {}", e);
        }
        process::exit(1);
    }
}

pub fn extract_isbn(dir: &Path, metadata: &mut Metadata) {
    for info in metadata {
        if !info.isbn.is_empty() {
            continue;
        }

        let path = dir.join(&info.file.path);

        if let Some(isbn) = DocumentOpener::new(EpubEngine::BuiltIn)
                                           .open(&path).and_then(|mut doc| doc.isbn()) {
            println!("ISBN {}", isbn);
            info.isbn = isbn;
        }
    }
}

pub fn retrieve(metadata: &mut Metadata, strict: bool) {
    for info in metadata {
        if !info.title.is_empty() {
            continue;
        }

        retriever_lookup_by_isbn(info, strict);
    }
}

#[inline]
pub fn retriever_lookup_by_isbn(info: &mut Info, strict: bool) {
    let terms = if info.isbn.is_empty() && !strict {
        label_from_path(&info.file.path)
    } else {
        info.isbn.clone()
    };

    if terms.is_empty() {
        return;
    }

    let url = format!("http://lookupbyisbn.com/Search/Book/{}/1", &terms);

    if let Ok(mut resp) = reqwest::get(&url) {
        if resp.status().is_success() {
            let mut content = String::new();
            resp.read_to_string(&mut content).unwrap();
            let re = Regex::new(r"(?xs)/Lookup/Book/.+?>
                                  ([^<]+)<.+?
                                  <u>([^<]+)</u>.+?
                                  <i>([^<]+)</i>.+?
                                  <i>([^<]+)</i>").unwrap();
            if let Some(caps) = re.captures(&content) {
                info.title = decode_entities(&caps[1]).into_owned();
                info.author = decode_entities(&caps[2]).into_owned();
                info.publisher = decode_entities(&caps[3]).into_owned();
                info.year = decode_entities(&caps[4]).into_owned();
                println!("{}", info.label());
            }
        } else {
            eprintln!("The request failed: {:?}.", resp.status());
        }
    }
}

#[inline]
pub fn retriever_amazon(info: &mut Info, _strict: bool) {
    let url = format!("https://www.amazon.com/s/?field-isbn={}", info.isbn);

    if let Ok(mut resp) = reqwest::get(&url) {
        if resp.status().is_success() {
            let mut content = String::new();
            resp.read_to_string(&mut content).unwrap();
            let re = Regex::new(r"s-access-title.+?<h3").unwrap();
            if let Some(mat) = re.find(&content) {
                let re = Regex::new(r">([^<]+)<").unwrap();
                for cap in re.captures_iter(&content[mat.start()..mat.end()]) {
                    if info.title.is_empty() {
                        info.title = decode_entities(&cap[1]).into_owned();
                    } else if info.year.is_empty() {
                        info.year = decode_entities(&cap[1]).into_owned();
                    } else {
                        info.author += &decode_entities(&cap[1]);
                    }
                }
                println!("{}", info.label());
            }
        } else {
            eprintln!("The request failed: {:?}.", resp.status());
        }
    }
}

pub fn consolidate(metadata: &mut Metadata) {
    for info in metadata {
        if info.subtitle.is_empty() {
            if let Some(index) = info.title.find(':') {
                let cur_title = info.title.clone();
                let (title, subtitle) = cur_title.split_at(index);
                info.title = title.trim_end().to_string();
                info.subtitle = subtitle[1..].trim_start().to_string();
            }
        }

        if info.language.is_empty() {
            info.title = titlecase(&info.title);
            info.subtitle = titlecase(&info.subtitle);
        }

        info.title = info.title.replace('\'', "’");
        info.subtitle = info.subtitle.replace('\'', "’");
        info.author = info.author.replace('\'', "’");
        if info.year.len() > 4 {
            info.year = info.year[..4].to_string();
        }
        info.series = info.series.replace('\'', "’");
        info.publisher = info.publisher.replace('\'', "’");
    }
}

pub fn rename(dir: &Path, metadata: &mut Metadata) {
    for info in metadata {
        let new_file_name = file_name_from_info(info);
        if !new_file_name.is_empty() {
            let old_rel_path = info.file.path.clone();
            let new_rel_path = old_rel_path.with_file_name(&new_file_name);
            if old_rel_path != new_rel_path {
                match fs::rename(dir.join(&old_rel_path), dir.join(&new_rel_path)) {
                    Err(e) => println!("Can't rename {} to {}: {}.",
                                       old_rel_path.display(),
                                       new_rel_path.display(), e),
                    Ok(..) => info.file.path = new_rel_path,
                }
            }
        }
    }
}

pub fn synchronize(src_dir: &Path, dest_dir: &Path, metadata: &Metadata) {
    for info in metadata {
        if let Some(parent) = info.file.path.parent() {
            let dest_parent = dest_dir.join(parent);
            if !dest_parent.exists() {
                if let Err(e) = fs::create_dir_all(&dest_parent) {
                    println!("Can't create {}: {}.",
                             dest_parent.display(), e);
                    continue;
                }
            }
        }

        let src = src_dir.join(&info.file.path);
        let dest = dest_dir.join(&info.file.path);

        if let Err(e) = fs::copy(&src, &dest) {
            println!("Can't copy {} to {}: {}.", src.display(), dest.display(), e);
        } else {
            println!("{} -> {}", src.display(), dest.display());
        }
    }
}

pub fn file_name_from_info(info: &Info) -> String {
    if info.title.is_empty() {
        return "".to_string();
    }
    let mut base = asciify(&info.title);
    if !info.subtitle.is_empty() {
        base = format!("{} - {}", base, asciify(&info.subtitle));
    }
    if !info.volume.is_empty() {
        base = format!("{} - {}", base, info.volume);
    }
    if !info.number.is_empty() && info.series.is_empty() {
        base = format!("{} - {}", base, info.number);
    }
    if !info.author.is_empty() {
        base = format!("{} - {}", base, asciify(&info.author));
    }
    base = format!("{}.{}", base, info.file.kind);
    base.replace("..", ".")
        .replace('/', " ")
        .replace('?', "")
        .replace('!', "")
        .replace(':', "")
}

pub fn label_from_path(path: &Path) -> String {
    path.file_stem().and_then(|p| p.to_str())
        .map(|t| t.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'', " ")).unwrap_or_default()
}
