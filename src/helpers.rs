extern crate serde_json;

use std::path::Path;
use std::fs::File;
use std::cmp::Ordering;
use serde::{Serialize, Deserialize};
use errors::*;

pub fn load_json<T, P: AsRef<Path>>(path: P) -> Result<T> where for<'a> T: Deserialize<'a> {
    let file = File::open(path).chain_err(|| "Can't open file.")?;
    serde_json::from_reader(file).chain_err(|| "Can't parse file.")
}

pub fn save_json<T, P: AsRef<Path>>(data: &T, path: P) -> Result<()> where T: Serialize {
    let file = File::create(path).chain_err(|| "Can't create data file.")?;
    serde_json::to_writer_pretty(file, data).chain_err(|| "Can't serialize data to file.")
}

pub fn combine_sort_methods<'a, T, F1, F2>(mut f1: F1, mut f2: F2) -> Box<FnMut(&T, &T) -> Ordering + 'a>
where F1: FnMut(&T, &T) -> Ordering + 'a,
      F2: FnMut(&T, &T) -> Ordering + 'a {
    Box::new(move |x, y| {
        f1(x, y).then_with(|| f2(x, y))
    })
}
