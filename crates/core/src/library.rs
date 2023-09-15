use std::fs::{self, File};
use std::str::FromStr;
use std::time::{SystemTime, Duration};
use std::path::{PathBuf, Path};
use std::collections::BTreeSet;
use std::io::{Error as IoError, ErrorKind};
use walkdir::WalkDir;
use indexmap::IndexMap;
use fxhash::{FxHashMap, FxHashSet, FxBuildHasher};
use chrono::{Local, NaiveDateTime};
use filetime::{FileTime, set_file_mtime, set_file_handle_times};
use anyhow::{Error, bail, format_err};
use crate::metadata::{Info, ReaderInfo, FileInfo, BookQuery, SimpleStatus, SortMethod};
use crate::metadata::{sort, sorter, extract_metadata_from_document};
use crate::settings::{LibraryMode, ImportSettings};
use crate::document::file_kind;
use crate::helpers::{Fingerprint, Fp, save_json, load_json, IsHidden};

pub const METADATA_FILENAME: &str = ".metadata.json";
pub const FAT32_EPOCH_FILENAME: &str = ".fat32-epoch";
pub const READING_STATES_DIRNAME: &str = ".reading-states";
pub const THUMBNAIL_PREVIEWS_DIRNAME: &str = ".thumbnail-previews";

pub struct Library {
    pub home: PathBuf,
    pub mode: LibraryMode,
    pub db: IndexMap<Fp, Info, FxBuildHasher>,
    pub paths: FxHashMap<PathBuf, Fp>,
    pub reading_states: FxHashMap<Fp, ReaderInfo>,
    pub modified_reading_states: FxHashSet<Fp>,
    pub has_db_changed: bool,
    pub fat32_epoch: SystemTime,
    pub sort_method: SortMethod,
    pub reverse_order: bool,
    pub show_hidden: bool,
}

impl Library {
    pub fn new<P: AsRef<Path>>(home: P, mode: LibraryMode) -> Result<Self, Error> {
        if let Err(e) = fs::create_dir(&home) {
            if e.kind() != ErrorKind::AlreadyExists {
                bail!(e);
            }
        }

        let path = home.as_ref().join(METADATA_FILENAME);
        let mut db;
        if mode == LibraryMode::Database {
            match load_json::<IndexMap<Fp, Info, FxBuildHasher>, _>(&path) {
                Err(e) => {
                    if e.downcast_ref::<IoError>().map(|e| e.kind()) != Some(ErrorKind::NotFound) {
                        bail!(e);
                    } else {
                        db = IndexMap::with_capacity_and_hasher(0, FxBuildHasher::default());
                    }
                },
                Ok(v) => db = v,
            }
        } else {
            db = IndexMap::with_capacity_and_hasher(0, FxBuildHasher::default());
        }

        let mut reading_states = FxHashMap::default();

        let path = home.as_ref().join(READING_STATES_DIRNAME);
        if let Err(e) = fs::create_dir(&path) {
            if e.kind() != ErrorKind::AlreadyExists {
                bail!(e);
            }
        }

        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(fp) = path.file_stem().and_then(|v| v.to_str())
                                  .and_then(|v| Fp::from_str(v).ok()) {
                if let Ok(reader_info) = load_json(path).map_err(|e| eprintln!("Can't load reading state: {:#}.", e)) {
                    if mode == LibraryMode::Database {
                        if let Some(info) = db.get_mut(&fp) {
                            info.reader = Some(reader_info);
                        } else {
                            eprintln!("Unknown fingerprint: {}.", fp);
                        }
                    } else {
                        reading_states.insert(fp, reader_info);
                    }
                }
            }
        }

        let path = home.as_ref().join(THUMBNAIL_PREVIEWS_DIRNAME);
        if !path.exists() {
            fs::create_dir(&path).ok();
        }

        let paths = if mode == LibraryMode::Database {
            db.iter().map(|(fp, info)| (info.file.path.clone(), *fp)).collect()
        } else {
            FxHashMap::default()
        };

        let path = home.as_ref().join(FAT32_EPOCH_FILENAME);
        if !path.exists() {
            let file = File::create(&path)?;
            let mtime = FileTime::from_unix_time(315_532_800, 0);
            set_file_handle_times(&file, None, Some(mtime))?;
        }

        let fat32_epoch = path.metadata()?.modified()?;

        let sort_method = SortMethod::Opened;

        Ok(Library {
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
        })
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
                                        .unwrap_or(path);
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
                        let secs = (*fp >> 32) as i64;
                        let nsecs = ((*fp & ((1<<32) - 1)) % 1_000_000_000) as u32;
                        let added = NaiveDateTime::from_timestamp_opt(secs, nsecs).unwrap();
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

    pub fn import(&mut self, settings: &ImportSettings) {
        if self.mode == LibraryMode::Filesystem {
            return;
        }

        for entry in WalkDir::new(&self.home).min_depth(1).into_iter()
                             .filter_entry(|e| !e.is_hidden()) {
            if entry.is_err() {
                continue;
            }

            let entry = entry.unwrap();
            if entry.file_type().is_dir() {
                continue;
            }

            let path = entry.path();
            let relat = path.strip_prefix(&self.home)
                            .unwrap_or(path);
            let md = entry.metadata().unwrap();
            let fp = md.fingerprint(self.fat32_epoch).unwrap();

            // The fp is know: update the path if it changed.
            if self.db.contains_key(&fp) {
                if relat != self.db[&fp].file.path {
                    println!("Update path for {}: {} → {}.",
                             fp, self.db[&fp].file.path.display(), relat.display());
                    self.paths.remove(&self.db[&fp].file.path);
                    self.paths.insert(relat.to_path_buf(), fp);
                    self.db[&fp].file.path = relat.to_path_buf();
                    self.has_db_changed = true;
                }
            // The path is known: update the fp.
            } else if let Some(fp2) = self.paths.get(relat).cloned() {
                println!("Update fingerprint for {}: {} → {}.", relat.display(), fp2, fp);
                let mut info = self.db.remove(&fp2).unwrap();
                if settings.sync_metadata && settings.metadata_kinds.contains(&info.file.kind) {
                    extract_metadata_from_document(&self.home, &mut info);
                }
                self.db.insert(fp, info);
                self.db[&fp].file.size = md.len();
                self.paths.insert(relat.to_path_buf(), fp);
                let rp1 = self.reading_state_path(fp2);
                let rp2 = self.reading_state_path(fp);
                fs::rename(rp1, rp2).ok();
                let tpp = self.thumbnail_preview_path(fp2);
                if tpp.exists() {
                    fs::remove_file(tpp).ok();
                }
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
                    println!("Update fingerprint for {}: {} → {}.", self.db[&nfp].file.path.display(), nfp, fp);
                    let info = self.db.remove(&nfp).unwrap();
                    self.db.insert(fp, info);
                    let rp1 = self.reading_state_path(nfp);
                    let rp2 = self.reading_state_path(fp);
                    fs::rename(rp1, rp2).ok();
                    let tp1 = self.thumbnail_preview_path(nfp);
                    let tp2 = self.thumbnail_preview_path(fp);
                    fs::rename(tp1, tp2).ok();
                    if relat != self.db[&fp].file.path {
                        println!("Update path for {}: {} → {}.",
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
                    println!("Add new entry: {}, {}.", fp, relat.display());
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
                    if settings.metadata_kinds.contains(&info.file.kind) {
                        extract_metadata_from_document(&self.home, &mut info);
                    }
                    self.db.insert(fp, info);
                    self.paths.insert(relat.to_path_buf(), fp);
                }

                self.has_db_changed = true;
            }
        }

        let home = &self.home;
        let len = self.db.len();

        self.db.retain(|fp, info| {
            let path = home.join(&info.file.path);
            if path.exists() {
                true
            } else {
                println!("Remove entry: {}, {}.", fp, info.file.path.display());
                false
            }
        });

        if self.db.len() != len {
            self.has_db_changed = true;
            let db = &self.db;
            self.paths.retain(|_, fp| db.contains_key(fp));
            self.modified_reading_states.retain(|fp| db.contains_key(fp));

            let reading_states_dir = home.join(READING_STATES_DIRNAME);
            let thumbnail_previews_dir = home.join(THUMBNAIL_PREVIEWS_DIRNAME);
            for entry in fs::read_dir(&reading_states_dir).unwrap()
                            .chain(fs::read_dir(&thumbnail_previews_dir).unwrap()) {
                if entry.is_err() {
                    continue;
                }
                let entry = entry.unwrap();
                if let Some(fp) = entry.path().file_stem()
                                       .and_then(|v| v.to_str())
                                       .and_then(|v| Fp::from_str(v).ok()) {
                    if !self.db.contains_key(&fp) {
                        fs::remove_file(entry.path()).ok();
                    }
                }
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

    pub fn rename<P: AsRef<Path>>(&mut self, path: P, file_name: &str) -> Result<(), Error> {
        let src = self.home.join(path.as_ref());

        let fp = self.paths.remove(path.as_ref()).or_else(|| {
           src.metadata().ok()
              .and_then(|md| md.fingerprint(self.fat32_epoch).ok())
        }).ok_or_else(|| format_err!("can't get fingerprint of {}", path.as_ref().display()))?;

        let mut dest = src.clone();
        dest.set_file_name(file_name);
        fs::rename(&src, &dest)?;

        if self.mode == LibraryMode::Database {
            let new_path = dest.strip_prefix(&self.home)?;
            self.paths.insert(new_path.to_path_buf(), fp);
            if let Some(info) = self.db.get_mut(&fp) {
                info.file.path = new_path.to_path_buf();
                self.has_db_changed = true;
            }
        }

        Ok(())
    }

    pub fn remove<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let full_path = self.home.join(path.as_ref());

        let fp = self.paths.get(path.as_ref()).cloned().or_else(|| {
           full_path.metadata().ok()
                    .and_then(|md| md.fingerprint(self.fat32_epoch).ok())
        }).ok_or_else(|| format_err!("can't get fingerprint of {}", path.as_ref().display()))?;

        if full_path.exists() {
            fs::remove_file(&full_path)?;
        }

        if let Some(parent) = full_path.parent() {
            if parent != self.home {
                fs::remove_dir(parent).ok();
            }
        }

        let rsp = self.reading_state_path(fp);
        if rsp.exists() {
            fs::remove_file(rsp)?;
        }

        let tpp = self.thumbnail_preview_path(fp);
        if tpp.exists() {
            fs::remove_file(tpp)?;
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

    pub fn copy_to<P: AsRef<Path>>(&mut self, path: P, other: &mut Library) -> Result<(), Error> {
        let src = self.home.join(path.as_ref());

        if !src.exists() {
            return Err(format_err!("can't copy non-existing file {}", path.as_ref().display()));
        }

        let md = src.metadata()?;
        let fp = self.paths.get(path.as_ref()).cloned()
                     .or_else(|| md.fingerprint(self.fat32_epoch).ok())
                     .ok_or_else(|| format_err!("can't get fingerprint of {}", path.as_ref().display()))?;

        let mut dest = other.home.join(path.as_ref());
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        if dest.exists() {
            let prefix = Local::now().format("%Y%m%d_%H%M%S ");
            let name = dest.file_name().and_then(|name| name.to_str())
                           .map(|name| prefix.to_string() + name)
                           .ok_or_else(|| format_err!("can't compute new name for {}", dest.display()))?;
            dest.set_file_name(name);
        }

        fs::copy(&src, &dest)?;
        let mtime = FileTime::from_last_modification_time(&md);
        set_file_mtime(&dest, mtime)?;

        let rsp_src = self.reading_state_path(fp);
        if rsp_src.exists() {
            let rsp_dest = other.reading_state_path(fp);
            fs::copy(&rsp_src, &rsp_dest)?;
        }

        let tpp_src = self.thumbnail_preview_path(fp);
        if tpp_src.exists() {
            let tpp_dest = other.thumbnail_preview_path(fp);
            fs::copy(&tpp_src, &tpp_dest)?;
        }

        if other.mode == LibraryMode::Database {
            let info = self.db.get(&fp).cloned()
                           .or_else(||
                               self.reading_states.get(&fp).cloned()
                                   .map(|reader_info| Info {
                                       file: FileInfo {
                                           size: md.len(),
                                           kind: file_kind(&dest).unwrap_or_default(),
                                           .. Default::default()
                                       },
                                       reader: Some(reader_info),
                                       .. Default::default()
                                   })
                           );
            if let Some(mut info) = info {
                let dest_path = dest.strip_prefix(&other.home)?;
                info.file.path = dest_path.to_path_buf();
                other.db.insert(fp, info);
                other.paths.insert(dest_path.to_path_buf(), fp);
                other.has_db_changed = true;
            }
        } else {
            let reader_info = self.reading_states.get(&fp).cloned()
                                  .or_else(|| self.db.get(&fp).cloned()
                                                  .and_then(|info| info.reader));
            if let Some(reader_info) = reader_info {
                other.reading_states.insert(fp, reader_info);
            }
        }

        other.modified_reading_states.insert(fp);

        Ok(())
    }

    pub fn move_to<P: AsRef<Path>>(&mut self, path: P, other: &mut Library) -> Result<(), Error> {
        let src = self.home.join(path.as_ref());

        if !src.exists() {
            return Err(format_err!("can't move non-existing file {}", path.as_ref().display()));
        }

        let md = src.metadata()?;
        let fp = self.paths.get(path.as_ref()).cloned()
                     .or_else(|| md.fingerprint(self.fat32_epoch).ok())
                     .ok_or_else(|| format_err!("can't get fingerprint of {}", path.as_ref().display()))?;

        let src = self.home.join(path.as_ref());
        let mut dest = other.home.join(path.as_ref());
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        if dest.exists() {
            let prefix = Local::now().format("%Y%m%d_%H%M%S ");
            let name = dest.file_name().and_then(|name| name.to_str())
                           .map(|name| prefix.to_string() + name)
                           .ok_or_else(|| format_err!("can't compute new name for {}", dest.display()))?;
            dest.set_file_name(name);
        }

        fs::rename(&src, &dest)?;

        let rsp_src = self.reading_state_path(fp);
        if rsp_src.exists() {
            let rsp_dest = other.reading_state_path(fp);
            fs::rename(&rsp_src, &rsp_dest)?;
        }

        let tpp_src = self.thumbnail_preview_path(fp);
        if tpp_src.exists() {
            let tpp_dest = other.thumbnail_preview_path(fp);
            fs::rename(&tpp_src, &tpp_dest)?;
        }

        if other.mode == LibraryMode::Database {
            let info = self.db.shift_remove(&fp)
                           .or_else(||
                               self.reading_states.remove(&fp)
                                   .map(|reader_info| Info {
                                       file: FileInfo {
                                           size: md.len(),
                                           kind: file_kind(&dest).unwrap_or_default(),
                                           .. Default::default()
                                       },
                                       reader: Some(reader_info),
                                       .. Default::default()
                                   })
                           );
            if let Some(mut info) = info {
                let dest_path = dest.strip_prefix(&other.home)?;
                info.file.path = dest_path.to_path_buf();
                other.db.insert(fp, info);
                self.paths.remove(path.as_ref());
                other.paths.insert(dest_path.to_path_buf(), fp);
                self.has_db_changed = true;
                other.has_db_changed = true;
            }
        } else {
            let reader_info = self.reading_states.remove(&fp)
                                  .or_else(|| self.db.shift_remove(&fp)
                                                  .and_then(|info| info.reader));
            if let Some(reader_info) = reader_info {
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
            return;
        }

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
                          .collect::<FxHashSet<Fp>>();

        self.reading_states.retain(|fp, _| {
            if fps.contains(fp) {
                true
            } else {
                println!("Remove reading state for {}.", fp);
                false
            }
        });
        self.modified_reading_states.retain(|fp| fps.contains(fp));

        let reading_states_dir = self.home.join(READING_STATES_DIRNAME);
        let thumbnail_previews_dir = self.home.join(THUMBNAIL_PREVIEWS_DIRNAME);
        for entry in fs::read_dir(&reading_states_dir).unwrap()
                        .chain(fs::read_dir(&thumbnail_previews_dir).unwrap()) {
            if entry.is_err() {
                continue;
            }
            let entry = entry.unwrap();
            if let Some(fp) = entry.path().file_stem()
                                   .and_then(|v| v.to_str())
                                   .and_then(|v| Fp::from_str(v).ok()) {
                if !fps.contains(&fp) {
                    fs::remove_file(entry.path()).ok();
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

    pub fn thumbnail_preview<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        if path.as_ref().starts_with(THUMBNAIL_PREVIEWS_DIRNAME) {
            self.home.join(path.as_ref())
        } else {
            let fp = self.paths.get(path.as_ref()).cloned().unwrap_or_else(|| {
                self.home.join(path.as_ref())
                    .metadata().unwrap()
                    .fingerprint(self.fat32_epoch).unwrap()
            });
            self.thumbnail_preview_path(fp)
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
                                              .get_or_insert_with(ReaderInfo::default);
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
                                          .or_insert_with(ReaderInfo::default);
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
                    eprintln!("Can't reload database: {:#}.", e);
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
                                  .and_then(|v| Fp::from_str(v).ok()) {
                if let Ok(reader_info) = load_json(path).map_err(|e| eprintln!("Can't load reading state: {:#}.", e)) {
                    if self.mode == LibraryMode::Database {
                        if let Some(info) = self.db.get_mut(&fp) {
                            info.reader = Some(reader_info);
                        } else {
                            eprintln!("Unknown fingerprint: {}.", fp);
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
                         .map_err(|e| eprintln!("Can't save reading state: {:#}.", e)).ok();
            }
        }

        self.modified_reading_states.clear();

        if self.has_db_changed {
            save_json(&self.db, self.home.join(METADATA_FILENAME))
                     .map_err(|e| eprintln!("Can't save database: {:#}.", e)).ok();
            self.has_db_changed = false;
        }
    }

    pub fn is_empty(&self) -> Option<bool> {
        if self.mode == LibraryMode::Database {
            Some(self.db.is_empty())
        } else {
            None
        }
    }

    fn reading_state_path(&self, fp: Fp) -> PathBuf {
        self.home
            .join(READING_STATES_DIRNAME)
            .join(format!("{}.json", fp))
    }

    fn thumbnail_preview_path(&self, fp: Fp) -> PathBuf {
        self.home
            .join(THUMBNAIL_PREVIEWS_DIRNAME)
            .join(format!("{}.png", fp))
    }
}
