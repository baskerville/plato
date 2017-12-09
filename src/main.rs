#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate downcast_rs;
extern crate unicode_normalization;
extern crate libc;
extern crate regex;
extern crate chrono;
extern crate fnv;
extern crate png;
extern crate isbn;
extern crate titlecase;
#[cfg(feature = "importer")]
extern crate reqwest;
#[cfg(feature = "importer")]
extern crate getopts;
#[cfg(feature = "importer")]
extern crate html_entities;

mod errors {
    error_chain!{
        links {
            Font(::font::Error, ::font::ErrorKind);
        }
    }
}

#[macro_use]
mod geom;
mod unit;
mod color;
mod device;
#[cfg(feature = "importer")]
mod importer;
mod frontlight;
mod framebuffer;
mod input;
mod gesture;
mod helpers;
mod document;
mod metadata;
mod symbolic_path;
mod settings;
mod view;
mod font;
mod app;

#[cfg(not(feature = "importer"))]
use app::run;
#[cfg(feature = "importer")]
use importer::run;

quick_main!(run);
