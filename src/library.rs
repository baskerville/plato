use std::fs::{self, File};
use std::time::{SystemTime, Duration};
use std::path::{PathBuf, Path};
use std::collections::BTreeSet;
use walkdir::WalkDir;
use indexmap::IndexMap;
use fxhash::{FxHashMap, FxHashSet, FxBuildHasher};
use chrono::{Local, TimeZone};
use filetime::{FileTime, set_file_handle_times};
use anyhow::{Error, format_err};
use crate::metadata::{Info, ReaderInfo, FileInfo, BookQuery, SimpleStatus, SortMethod};
use crate::metadata::{sort, sorter, extract_metadata_from_epub};
use crate::settings::{LibraryMode, ImportSettings};
use crate::document::file_kind;
use crate::helpers::{Fingerprint, save_json, load_json, IsHidden};

pub const METADATA_FILENAME: &str = ".metadata.json";
pub const FAT32_EPOCH_FILENAME: &str = ".fat32-epoch";
pub const READING_STATES_DIRNAME: &str = ".reading-states";

pub struct Library {
    pub home: PathBuf,
    pub mode: LibraryMode,
    pub db: IndexMap<u64, Info, FxBuildHasher>,
    pub paths: FxHashMap<PathBuf, u64>,
    pub reading_states: FxHashMap<u64, ReaderInfo>,
    pub modified_reading_states: FxHashSet<u64>,
    pub has_db_changed: bool,
    pub fat32_epoch: SystemTime,
    pub sort_method: SortMethod,
    pub reverse_order: bool,
    pub show_hidden: bool,
}

impl Library {
    pub fn new<P: AsRef<Path>>(home: P, mode: LibraryMode) -> Self {
        let mut db: IndexMap<u64, Info, FxBuildHasher> = if mode == LibraryMode::Database {
            let path = home.as_ref().join(METADATA_FILENAME);
            match load_json(&path) {
                Err(e) => {
                    if path.exists() {
                        eprintln!("{}", e);
                    }
                    IndexMap::with_capacity_and_hasher(0, FxBuildHasher::default())
                },
                Ok(v) => v,
            }
        } else {
            IndexMap::with_capacity_and_hasher(0, FxBuildHasher::default())
        };

        let mut reading_states = FxHashMap::default();

        let path = home.as_ref().join(READING_STATES_DIRNAME);
        if !path.exists() {
            fs::create_dir(&path).ok();
        }

        for entry in fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if let Some(fp) = path.file_stem().and_then(|v| v.to_str())
                                  .and_then(|v| u64::from_str_radix(v, 16).ok()) {
                if let Ok(reader_info) = load_json(path).map_err(|e| eprintln!("{}", e)) {
                    if mode == LibraryMode::Database {
                        if let Some(info) = db.get_mut(&fp) {
                            info.reader = Some(reader_info);
                        } else {
                            eprintln!("Unknown fingerprint: {:016X}.", fp);
                        }
                    } else {
                        reading_states.insert(fp, reader_info);
                    }
                }
            }
        }

        let paths = if mode == LibraryMode::Database {
            db.iter().map(|(fp, info)| (info.file.path.clone(), *fp)).collect()
        } else {
            FxHashMap::default()
        };

        let path = home.as_ref().join(FAT32_EPOCH_FILENAME);
        if !path.exists() {
            let file = File::create(&path).unwrap();
            let mtime = FileTime::from_unix_time(315_532_800, 0);
            set_file_handle_times(&file, None, Some(mtime))
                    .map_err(|e| eprintln!("{}", e)).ok();
        }

        let fat32_epoch = path.metadata().unwrap().modified().unwrap();

        let sort_method = SortMethod::Opened;

        Library {
            home: home.as_ref().to_path_buf(),
            mode,
            db,
            paths,
            reading_states,
            modified_reading_states: FxHashSet::default(),
            has_db_changed: false,
            fat32_epoch,
            sort_method,
            reverse_order: sort_method.reverse_order(),
            show_hidden: false,
        }
    }

    pub fn list<P: AsRef<Path>>(&self, prefix: P, query: Option<&BookQuery>, skip_files: bool) -> (Vec<Info>, BTreeSet<PathBuf>) {
        let mut dirs = BTreeSet::new();
        let mut files = Vec::new();

        match self.mode {
            LibraryMode::Database => {
                let relat_prefix = prefix.as_ref().strip_prefix(&self.home)
                                         .unwrap_or_else(|_| prefix.as_ref());
                for (_, info) in self.db.iter() {
                    if let Ok(relat) = info.file.path.strip_prefix(relat_prefix) {
                        let mut compos = relat.components();
                        let mut first = compos.next();
                        // `first` is a file.
                        if compos.next().is_none() {
                            first = None;
                        }
                        if let Some(child) = first {
                            dirs.insert(prefix.as_ref().join(child.as_os_str()));
                        }
                        if skip_files {
                            continue;
                        }
                        if query.map_or(true, |q| q.is_match(info)) {
                            files.push(info.clone());
                        }
                    }
                }
            },
            LibraryMode::Filesystem => {
                if !prefix.as_ref().is_dir() {
                    return (files, dirs);
                }

                let max_depth = if query.is_some() {
                    usize::MAX
                } else {
                    1
                };

                for entry in WalkDir::new(prefix.as_ref())
                                     .min_depth(1)
                                     .max_depth(max_depth)
                                     .into_iter()
                                     .filter_entry(|e| self.show_hidden || !e.is_hidden()) {
                    if entry.is_err() {
                        continue;
                    }
                    let entry = entry.unwrap();
                    let path = entry.path();

                    if path.is_dir() {
                        if entry.depth() == 1 {
                            dirs.insert(path.to_path_buf());
                        }
                    } else {
                        let relat = path.strip_prefix(&self.home)
                                        .unwrap_or_else(|_| path.as_ref());
                        if skip_files || query.map_or(false, |q| {
                            relat.to_str().map_or(true, |s| !q.is_simple_match(s))
                        }) {
                            continue;
                        }

                        let kind = file_kind(&path).unwrap_or_default();
                        let md = entry.metadata().unwrap();
                        let size = md.len();
                        let fp = md.fingerprint(self.fat32_epoch).unwrap();
                        let file = FileInfo {
                            path: relat.to_path_buf(),
                            kind,
                            size,
                        };
                        let secs = (fp >> 32) as i64;
                        let nsecs = ((fp & ((1<<32) - 1)) % 1_000_000_000) as u32;
                        let added = Local.timestamp(secs, nsecs);
                        let info = Info {
                            file,
                            added,
                            reader: self.reading_states.get(&fp).cloned(),
                            .. Default::default()
                        };

                        files.push(info);
                    }
                }

                sort(&mut files, self.sort_method, self.reverse_order);
            },
        }

        (files, dirs)
    }

    pub fn import<P: AsRef<Path>>(&mut self, prefix: P, settings: &ImportSettings) {
        if self.mode == LibraryMode::Filesystem {
            return;
        }

        for entry in WalkDir::new(prefix.as_ref()).min_depth(1)
                             .into_iter()
                             .filter_entry(|e| settings.traverse_hidden || !e.is_hidden()) {
            if entry.is_err() {
                continue;
            }

            let entry = entry.unwrap();
            let path = entry.path();
            let relat = path.strip_prefix(&self.home).unwrap_or_else(|_| path);
            let md = entry.metadata().unwrap();
            let fp = md.fingerprint(self.fat32_epoch).unwrap();

            // The fp is know: update the path if it changed.
            if self.db.contains_key(&fp) {
                if relat != self.db[&fp].file.path {
                    println!("Update path for {:016X}: {} → {}.",
                             fp, self.db[&fp].file.path.display(), relat.display());
                    self.paths.remove(&self.db[&fp].file.path);
                    self.paths.insert(relat.to_path_buf(), fp);
                    self.db[&fp].file.path = relat.to_path_buf();
                    self.has_db_changed = true;
                }
            // The path is known: update the fp.
            } else if let Some(fp2) = self.paths.get(relat).cloned() {
                println!("Update fingerprint for {}: {:016X} → {:016X}.", relat.display(), fp2, fp);
                let info = self.db.remove(&fp2).unwrap();
                self.db.insert(fp, info);
                self.db[&fp].file.size = md.len();
                self.paths.insert(relat.to_path_buf(), fp);
                let rp1 = self.reading_state_path(fp2);
                let rp2 = self.reading_state_path(fp);
                fs::rename(rp1, rp2).ok();
                self.has_db_changed = true;
            } else {
                let fp1 = self.fat32_epoch.checked_sub(Duration::from_secs(1))
                              .and_then(|epoch| md.fingerprint(epoch).ok()).unwrap_or(fp);
                let fp2 = self.fat32_epoch.checked_add(Duration::from_secs(1))
                              .and_then(|epoch| md.fingerprint(epoch).ok()).unwrap_or(fp);

                let nfp = if fp1 != fp && self.db.contains_key(&fp1) {
                    Some(fp1)
                } else if fp2 != fp && self.db.contains_key(&fp2) {
                    Some(fp2)
                } else {
                    None
                };

                // On a FAT32 file system, the modification time has a two-second precision.
                // This might be the reason why the modification time of a file can sometimes
                // drift by one second, when the file is created within an operating system
                // and moved within another.
                if let Some(nfp) = nfp {
                    println!("Update fingerprint for {}: {:016X} → {:016X}.", self.db[&nfp].file.path.display(), nfp, fp);
                    let info = self.db.remove(&nfp).unwrap();
                    self.db.insert(fp, info);
                    let rp1 = self.reading_state_path(nfp);
                    let rp2 = self.reading_state_path(fp);
                    fs::rename(rp1, rp2).ok();
                    if relat != self.db[&fp].file.path {
                        println!("Update path for {:016X}: {} → {}.",
                                 fp, self.db[&fp].file.path.display(), relat.display());
                        self.paths.remove(&self.db[&fp].file.path);
                        self.paths.insert(relat.to_path_buf(), fp);
                        self.db[&fp].file.path = relat.to_path_buf();
                    }
                // We found a new file: add it to the db.
                } else {
                    let kind = file_kind(&path).unwrap_or_default();
                    if !settings.allowed_kinds.contains(&kind) {
                        continue;
                    }
                    println!("Add new entry: {:016X}, {}.", fp, relat.display());
                    let size = md.len();
                    let file = FileInfo {
                        path: relat.to_path_buf(),
                        kind,
                        size,
                    };
                    let mut info = Info {
                        file,
                        .. Default::default()
                    };
                    if settings.extract_epub_metadata {
                        extract_metadata_from_epub(prefix.as_ref(), &mut info);
                    }
                    self.db.insert(fp, info);
                    self.paths.insert(relat.to_path_buf(), fp);
                }

                self.has_db_changed = true;
            }
        }
    }

    pub fn add_document(&mut self, info: Info) {
        if self.mode == LibraryMode::Filesystem {
            return;
        }

        let path = self.home.join(&info.file.path);
        let md = path.metadata().unwrap();
        let fp = md.fingerprint(self.fat32_epoch).unwrap();

        self.paths.insert(info.file.path.clone(), fp);
        self.db.insert(fp, info);
        self.has_db_changed = true;
    }

    pub fn remove<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let full_path = self.home.join(path.as_ref());

        let fp = self.paths.get(path.as_ref()).cloned().or_else(|| {
           full_path.metadata().ok()
                    .and_then(|md| md.fingerprint(self.fat32_epoch).ok())
        }).ok_or_else(|| format_err!("Can't get fingerprint of {}.", path.as_ref().display()))?;

        if full_path.exists() {
            fs::remove_file(&full_path)?;
            if let Some(parent) = full_path.parent() {
                if parent != self.home {
                    fs::remove_dir(parent).ok();
                }
            }
        }

        let rsp = self.reading_state_path(fp);
        if rsp.exists() {
            fs::remove_file(rsp)?;
        }

        if self.mode == LibraryMode::Database {
            self.paths.remove(path.as_ref());
            if self.db.shift_remove(&fp).is_some() {
                self.has_db_changed = true;
            }
        } else {
            self.reading_states.remove(&fp);
        }

        self.modified_reading_states.remove(&fp);

        Ok(())
    }

    pub fn move_to<P: AsRef<Path>>(&mut self, path: P, other: &mut Library) -> Result<(), Error> {
        if !self.home.join(path.as_ref()).exists() {
            return Err(format_err!("Can't move non-existing file {}.", path.as_ref().display()));
        }

        let fp = self.paths.get(path.as_ref()).cloned().or_else(|| {
            self.home.join(path.as_ref())
                .metadata().ok()
                .and_then(|md| md.fingerprint(self.fat32_epoch).ok())
        }).ok_or_else(|| format_err!("Can't get fingerprint of {}.", path.as_ref().display()))?;

        let src = self.home.join(path.as_ref());
        let mut dest = other.home.join(path.as_ref());
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        if dest.exists() {
            let prefix = Local::now().format("%Y%m%d_%H%M%S ");
            let name = dest.file_name().and_then(|name| name.to_str())
                           .map(|name| prefix.to_string() + name)
                           .ok_or_else(|| format_err!("Can't compute new name for {}.", dest.display()))?;
            dest.set_file_name(name);
        }

        fs::rename(&src, &dest)?;

        let rsp_src = self.reading_state_path(fp);
        if rsp_src.exists() {
            let rsp_dest = other.reading_state_path(fp);
            fs::rename(&rsp_src, &rsp_dest)?;
        }

        if self.mode == LibraryMode::Database {
            if let Some(mut info) = self.db.shift_remove(&fp) {
                let dest_path = dest.strip_prefix(&other.home)?;
                info.file.path = dest_path.to_path_buf();
                other.db.insert(fp, info);
                self.paths.remove(path.as_ref());
                other.paths.insert(dest_path.to_path_buf(), fp);
                self.has_db_changed = true;
                other.has_db_changed = true;
            }
        } else {
            if let Some(reader_info) = self.reading_states.remove(&fp) {
                other.reading_states.insert(fp, reader_info);
            }
        }

        if self.modified_reading_states.remove(&fp) {
            other.modified_reading_states.insert(fp);
        }

        Ok(())
    }

    pub fn clean_up(&mut self) {
        if self.mode == LibraryMode::Database {
            let home = &self.home;
            let len = self.db.len();
            self.db.retain(|fp, info| {
                let path = home.join(&info.file.path);
                if path.exists() {
                    true
                } else {
                    println!("Remove entry: {:016X}, {}.", fp, info.file.path.display());
                    false
                }
            });
            self.paths.retain(|path, _| home.join(path).exists());
            let db = &self.db;
            self.modified_reading_states.retain(|fp| db.contains_key(fp));

            if self.db.len() != len {
                self.has_db_changed = true;
            }

            let path = home.join(READING_STATES_DIRNAME);
            for entry in fs::read_dir(&path).unwrap() {
                if entry.is_err() {
                    continue;
                }
                let entry = entry.unwrap();
                if let Some(fp) = entry.path().file_stem()
                                       .and_then(|v| v.to_str())
                                       .and_then(|v| u64::from_str_radix(v, 16).ok()) {
                    if !self.db.contains_key(&fp) {
                        fs::remove_file(entry.path()).ok();
                    }
                }
            }
        } else {
            let fps = WalkDir::new(&self.home)
                              .min_depth(1).into_iter()
                              .filter_map(|entry| entry.ok())
                              .filter_map(|entry| {
                                  if entry.file_type().is_dir() {
                                      None
                                  } else {
                                      Some(entry.metadata().unwrap()
                                                .fingerprint(self.fat32_epoch).unwrap())
                                  }
                              })
                              .collect::<FxHashSet<u64>>();
            let path = self.home.join(READING_STATES_DIRNAME);
            for entry in fs::read_dir(&path).unwrap() {
                if entry.is_err() {
                    continue;
                }
                let entry = entry.unwrap();
                if let Some(fp) = entry.path().file_stem()
                                       .and_then(|v| v.to_str())
                                       .and_then(|v| u64::from_str_radix(v, 16).ok()) {
                    if !fps.contains(&fp) {
                        println!("Remove reading state for {:016X}.", fp);
                        self.reading_states.remove(&fp);
                        self.modified_reading_states.remove(&fp);
                        fs::remove_file(entry.path()).ok();
                    }
                }
            }
        }
    }

    pub fn sort(&mut self, sort_method: SortMethod, reverse_order: bool) {
        self.sort_method = sort_method;
        self.reverse_order = reverse_order;

        if self.mode == LibraryMode::Filesystem {
            return;
        }

        let sort_fn = sorter(sort_method);

        if reverse_order {
            self.db.sort_by(|_, a, _, b| sort_fn(a, b).reverse());
        } else {
            self.db.sort_by(|_, a, _, b| sort_fn(a, b));
        }
    }

    pub fn apply<F>(&mut self, f: F) where F: Fn(&Path, &mut Info) {
        if self.mode == LibraryMode::Filesystem {
            return;
        }

        for (_, info) in &mut self.db {
            f(&self.home, info);
        }

        self.has_db_changed = true;
    }

    pub fn sync_reader_info<P: AsRef<Path>>(&mut self, path: P, reader: &ReaderInfo) {
        let fp = self.paths.get(path.as_ref()).cloned().unwrap_or_else(|| {
            self.home.join(path.as_ref())
                .metadata().unwrap()
                .fingerprint(self.fat32_epoch).unwrap()
        });
        self.modified_reading_states.insert(fp);
        match self.mode {
            LibraryMode::Database => {
                if let Some(info) = self.db.get_mut(&fp) {
                    info.reader = Some(reader.clone());
                }
            },
            LibraryMode::Filesystem => {
                self.reading_states.insert(fp, reader.clone());
            },
        }
    }

    pub fn set_status<P: AsRef<Path>>(&mut self, path: P, status: SimpleStatus) {
        let fp = self.paths.get(path.as_ref()).cloned().unwrap_or_else(|| {
            self.home.join(path.as_ref())
                .metadata().unwrap()
                .fingerprint(self.fat32_epoch).unwrap()
        });
        if self.mode == LibraryMode::Database {
            match status {
                SimpleStatus::New => {
                    if let Some(info) = self.db.get_mut(&fp) {
                        info.reader = None;
                    }
                    fs::remove_file(self.reading_state_path(fp)).ok();
                    self.modified_reading_states.remove(&fp);
                },
                SimpleStatus::Reading | SimpleStatus::Finished => {
                    if let Some(info) = self.db.get_mut(&fp) {
                        let reader_info = info.reader
                                              .get_or_insert_with(|| ReaderInfo::default());
                        reader_info.finished = status == SimpleStatus::Finished;
                        self.modified_reading_states.insert(fp);
                    }
                },
            }
        } else {
            match status {
                SimpleStatus::New => {
                    self.reading_states.remove(&fp);
                    fs::remove_file(self.reading_state_path(fp)).ok();
                    self.modified_reading_states.remove(&fp);
                },
                SimpleStatus::Reading | SimpleStatus::Finished => {
                    let reader_info = self.reading_states.entry(fp)
                                          .or_insert_with(|| ReaderInfo::default());
                    reader_info.finished = status == SimpleStatus::Finished;
                    self.modified_reading_states.insert(fp);
                },
            }
        }
    }

    pub fn reload(&mut self) {
        if self.mode == LibraryMode::Database {
            let path = self.home.join(METADATA_FILENAME);

            match load_json(&path) {
                Err(e) => {
                    if path.exists() {
                        eprintln!("Can't load {}: {}", path.display(), e);
                    }
                    return;
                },
                Ok(v) => {
                    self.db = v;
                    self.has_db_changed = false;
                },
            }
        }

        let path = self.home.join(READING_STATES_DIRNAME);

        self.modified_reading_states.clear();
        if self.mode == LibraryMode::Filesystem {
            self.reading_states.clear();
        }

        for entry in fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if let Some(fp) = path.file_stem().and_then(|v| v.to_str())
                                  .and_then(|v| u64::from_str_radix(v, 16).ok()) {
                if let Ok(reader_info) = load_json(path).map_err(|e| eprintln!("{}", e)) {
                    if self.mode == LibraryMode::Database {
                        if let Some(info) = self.db.get_mut(&fp) {
                            info.reader = Some(reader_info);
                        } else {
                            eprintln!("Unknown fingerprint: {:016X}.", fp);
                        }
                    } else {
                        self.reading_states.insert(fp, reader_info);
                    }
                }
            }
        }

        if self.mode == LibraryMode::Database {
            self.paths = self.db.iter().map(|(fp, info)| (info.file.path.clone(), *fp)).collect();
        }
    }

    pub fn flush(&mut self) {
        for fp in &self.modified_reading_states {
            let reader_info = if self.mode == LibraryMode::Database {
                self.db.get(fp).and_then(|info| info.reader.as_ref())
            } else {
                self.reading_states.get(fp)
            };
            if let Some(reader_info) = reader_info {
                save_json(reader_info, self.reading_state_path(*fp))
                         .map_err(|e| eprintln!("{}", e)).ok();
            }
        }

        self.modified_reading_states.clear();

        if self.has_db_changed {
            save_json(&self.db, self.home.join(METADATA_FILENAME))
                     .map_err(|e| eprintln!("{}", e)).ok();
            self.has_db_changed = false;
        }
    }

    fn reading_state_path(&self, fp: u64) -> PathBuf {
        self.home
            .join(READING_STATES_DIRNAME)
            .join(format!("{:016X}.json", fp))
    }
}
