#![recursion_limit = "1024"]

extern crate rand;
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
extern crate glob;
extern crate fnv;
extern crate png;
extern crate isbn;
extern crate titlecase;

mod errors {
    error_chain!{
        foreign_links {
            Io(::std::io::Error);
            ParseInt(::std::num::ParseIntError);
        }
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
mod framebuffer;
mod frontlight;
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

use app::run;

quick_main!(run);
