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
mod translate;
mod document;
mod library;
mod metadata;
mod symbolic_path;
mod rtc;
mod settings;
mod view;
mod font;
mod app;

use anyhow::Error;
use crate::app::run;

fn main() -> Result<(), Error> {
    run()?;
    Ok(())
}
