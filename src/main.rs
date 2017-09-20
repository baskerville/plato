#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate downcast_rs;
extern crate libc;
extern crate reqwest;
extern crate regex;
extern crate fnv;
extern crate unicode_normalization;
extern crate isbn;
extern crate titlecase;
extern crate chrono;
extern crate png;

mod errors {
    error_chain!{}
}

#[macro_use]
mod geom;
mod unit;
mod color;
mod device;
mod backlight;
mod framebuffer;
mod input;
mod gesture;
mod document;
mod metadata;
mod settings;
mod importer;
mod view;
mod font;
mod app;

use app::run;

quick_main!(run);
