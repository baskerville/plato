
// extern crate libremarkable;
// use libremarkable::fb;
// use libremarkable::fbdraw::FramebufferDraw;
// use libremarkable::mxc_types::{VarScreeninfo,FixScreeninfo};

use std::ptr;
use std::path::Path;
use std::io;
use std::fs::{OpenOptions, File};
use std::slice;
use std::borrow::Cow;
use std::os::unix::io::AsRawFd;
use std::ops::Drop;
use libc::ioctl;
use png::HasParameters;
use geom::Rectangle;
use framebuffer::{UpdateMode, Framebuffer};
use framebuffer::mxcfb_sys::*;
use errors::*;


pub struct RemarkableFramebuffer {
	// fb: libremarkable::fb::Framebuffer
}




// pub trait FramebufferIO {
//     fn write_frame(&mut self, frame: &[u8]);
//     fn write_pixel(&mut self, y: usize, x: usize, v: u8);
//     fn read_pixel(&mut self, y: usize, x: usize) -> u8;
//     fn read_offset(&mut self, ofst: isize) -> u8;
// }

//    fn wait_refresh_complete(&mut self, marker: u32);
//     fn refresh(
    //     &mut self,
    //     region: &mxc_types::mxcfb_rect,
    //     update_mode: mxc_types::update_mode,
    //     waveform_mode: mxc_types::waveform_mode,
    //     temperature: mxc_types::display_temp,
    //     dither_mode: mxc_types::dither_mode,
    //     quant_bit: i32,
    //     flags: u32,
    // ) -> u32;



impl Framebuffer for RemarkableFramebuffer {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        print!("set_pixel {} {} {}", x, y, color);
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        print!("set_blended_pixel {} {} {} {}", x, y, color, alpha);

    }
    fn invert_region(&mut self, rect: &Rectangle) {

    }
    fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> Result<u32> {
    	// print!("update {} {}", rect, mode);
        print!("update");
        // fb.refresh(rmRect, rmUpdateMode, rmWaveformMode, tempMode, ditherMode, )
        Ok(2)
    }
    fn wait(&self, token: u32) -> Result<i32> {
        print!("wait {}", token);

    	// fb.wait_refresh_complete(token)
        Ok(2)
    }
    fn save(&self, path: &str) -> Result<()> {
        print!("save {}", path);

        Ok( () )
    }
    fn toggle_inverted(&mut self) {
        print!("toggle_inverted");

    }
    fn toggle_monochrome(&mut self) {
        print!("toggle_monochrome");

    }

}

impl RemarkableFramebuffer {
    pub fn new() -> Result<RemarkableFramebuffer> {
    	// let framebuffer = Box::new(libremarkable::fb::Framebuffer::new("/dev/fb0"));
        // let yres = framebuffer.var_screen_info.yres;
        // let xres = framebuffer.var_screen_info.xres;
        Ok(RemarkableFramebuffer { 
            // framebuffer 
        })
    }
}