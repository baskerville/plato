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
#[macro_use]
extern crate geom;
extern crate color;
extern crate framebuffer;
extern crate font;
extern crate unicode_normalization;
extern crate libc;
extern crate regex;
extern crate chrono;
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

mod unit;
mod device;
mod frontlight;
mod battery;
mod input;
mod gesture;
mod helpers;
mod document;
mod metadata;
mod symbolic_path;
mod settings;
mod view;
mod app;

use app::run;

quick_main!(run);
