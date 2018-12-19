use std::fs;
use std::path::PathBuf;
use std::collections::VecDeque;
use failure::{Error, format_err};
use rand::{Rng, thread_rng};
use serde_derive::{Serialize, Deserialize};
use crate::metadata::TRASH_NAME;
use fnv::FnvHashSet;
use crate::helpers::{load_json, save_json};
use crate::app::Context;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct TrashEntry {
    name: String,
    path: PathBuf,
    size: u64,
}

impl Default for TrashEntry {
    fn default() -> Self {
        TrashEntry {
            name: String::default(),
            path: PathBuf::default(),
            size: 0,
        }
    }
}

type Trash = VecDeque<Vec<TrashEntry>>;

const CONTENTS_NAME: &str = "contents.json";
const SIZE_LIMIT: u64 = 32 * 1024 * 1024;
const MIN_PACKETS: usize = 8;

pub fn trash(paths: &FnvHashSet<PathBuf>, context: &mut Context) -> Result<(), Error> {
    let library_path = &context.settings.library_path;
    let trash_path = library_path.join(TRASH_NAME);
    let contents_path = trash_path.join(CONTENTS_NAME);

    if trash_path.exists() {
        if trash_path.metadata().map(|m| !m.is_dir()).unwrap_or(false) {
            return Err(format_err!("{} exists and isn't a directory.",
                                   trash_path.display()));
        }
    } else {
        fs::create_dir(&trash_path)?;
    }

    let contents = load_json::<Trash, _>(&contents_path);

    if contents.is_err() && contents_path.exists() {
        return Err(contents.unwrap_err());
    }

    let mut contents = contents.unwrap_or_default();
    let mut entries = Vec::new();
    let mut rng = thread_rng();

    for path in paths {
        let mut name = crockford::encode(rng.gen());
        let mut dest = trash_path.join(&name);

        for _ in 0..3 {
            if !dest.exists() {
                break;
            }
            name = crockford::encode(rng.gen());
            dest = trash_path.join(&name);
        }

        let src = library_path.join(path);
        let size = src.metadata()?.len();

        entries.push(TrashEntry { name, path: path.clone(), size });
        fs::rename(src, dest)?;
    }

    contents.push_front(entries);

    let mut total_size: u64 = contents.iter().flat_map(|e| e).map(|e| e.size).sum();
    let mut packets_count = contents.len();

    if total_size > SIZE_LIMIT && packets_count >= MIN_PACKETS {
        while let Some(mut entries) = contents.pop_back() {
            while let Some(entry) = entries.pop() {
                if fs::remove_file(trash_path.join(&entry.name))
                      .map_err(|e| eprintln!("Can't remove {}: {}", &entry.name, e)).is_err() {
                    entries.push(entry);
                    break;
                } else {
                    total_size -= entry.size;
                }
            }

            if !entries.is_empty() {
                contents.push_back(entries);
                break;
            }

            packets_count -= 1;

            if total_size <= SIZE_LIMIT || packets_count < MIN_PACKETS {
                break;
            }
        }
    }

    save_json(&contents, &contents_path)?;

    Ok(())
}

pub fn untrash(context: &mut Context) -> Result<(), Error> {
    let library_path = &context.settings.library_path;
    let trash_path = library_path.join(TRASH_NAME);
    let contents_path = trash_path.join(CONTENTS_NAME);
    let mut contents = load_json::<Trash, _>(&contents_path)?;

    if let Some(mut entries) = contents.pop_front() {
        while let Some(entry) = entries.pop() {
            if fs::rename(trash_path.join(&entry.name), library_path.join(&entry.path))
                  .map_err(|e| eprintln!("Can't restore {}: {}", &entry.name, e)).is_err() {
              entries.push(entry);
              break;
            }
        }

        if !entries.is_empty() {
            contents.push_front(entries);
        }
    }

    save_json(&contents, &contents_path)?;

    Ok(())
}

pub fn empty(context: &Context) -> Result<(), Error> {
    let library_path = &context.settings.library_path;
    let trash_path = library_path.join(TRASH_NAME);
    let contents_path = trash_path.join(CONTENTS_NAME);
    let mut contents = load_json::<Trash, _>(&contents_path)?;

    for entries in &contents {
        for entry in entries {
            fs::remove_file(trash_path.join(&entry.name))
               .map_err(|e| eprintln!("Can't remove {}: {}", &entry.name, e)).ok();
        }
    }

    contents.clear();
    save_json(&contents, &contents_path)?;

    Ok(())
}

pub fn is_empty(context: &Context) -> bool {
    let library_path = &context.settings.library_path;
    let trash_path = library_path.join(TRASH_NAME);
    if !trash_path.exists() {
        return true;
    }
    let count = fs::read_dir(trash_path)
                   .map(|dir| dir.into_iter().count()).unwrap_or(0);
    count < 2
}
