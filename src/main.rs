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
mod dictionary;
mod document;
mod metadata;
mod symbolic_path;
mod rtc;
mod settings;
mod trash;
mod view;
mod font;
mod app;

use std::process;
use crate::app::run;

fn main() {
    if let Err(e) = run() {
        for e in e.iter_chain() {
            eprintln!("plato: {}", e);
        }
        process::exit(1);
    }
}
