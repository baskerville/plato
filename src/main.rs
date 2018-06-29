extern crate rand;
#[macro_use] extern crate failure;
#[macro_use] extern crate failure_derive;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate toml;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate bitflags;
#[macro_use] extern crate downcast_rs;
extern crate unicode_normalization;
extern crate libc;
extern crate regex;
extern crate chrono;
extern crate glob;
extern crate fnv;
extern crate png;
extern crate isbn;
extern crate titlecase;

#[macro_use] mod geom;
mod unit;
mod color;
mod device;
mod framebuffer;
mod frontlight;
mod lightsensor;
mod battery;
mod input;
mod gesture;
mod helpers;
mod document;
mod metadata;
mod symbolic_path;
mod settings;
mod trash;
mod view;
mod font;
mod app;

use std::process;
use app::run;

fn main() {
    if let Err(e) = run() {
        for e in e.causes() {
            eprintln!("plato: {}", e);
        }
        process::exit(1);
    }
}
