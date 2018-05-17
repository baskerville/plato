//#![feature(remarkable)]
extern crate libremarkable;
use self::libremarkable::framebuffer as remarkable_fb;
use self::libremarkable::framebuffer::{FramebufferIO, FramebufferDraw, FramebufferRefresh, FramebufferBase};
// use libremarkable::fb as remarkable_fb;
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

use self::libremarkable::framebuffer::common::*;
use self::libremarkable::framebuffer::refresh::PartialRefreshMode;


pub struct RemarkableFramebuffer<'a>  {
	 fb: remarkable_fb::core::Framebuffer<'a>
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



impl<'a> Framebuffer for RemarkableFramebuffer<'a> {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
//        print!("-set_pixel {} {} {}\n", x, y, color);
        self.fb.write_pixel(y as usize, x as usize, color::NATIVE_COMPONENTS(color,color,color,color));
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        if alpha == 1.0 {
            self.set_pixel(x, y, color);
            return;
        }
        let dst_color = self.fb.read_pixel(y as usize, x as usize);
        let dst_color = dst_color.as_native();
        let (dst_r, dst_g, dst_b) = (dst_color[0], dst_color[1], dst_color[2]);
        let src_alpha = color as f32 * alpha;
        let r = src_alpha + (1.0 - alpha) * dst_r as f32;
        let g = src_alpha + (1.0 - alpha) * dst_g as f32;
        let b = src_alpha + (1.0 - alpha) * dst_b as f32;
        let a = (r+g+b)/3.0;
        //we ignoring alpha of pixel read
//        print!("setting blended color: dst: {} {} {}  src: {}   res: {} {} {} {} \n" , dst_r, dst_g, dst_b, src_alpha, r, g, b, a);
        self.fb.write_pixel(y as usize, x as usize, color::NATIVE_COMPONENTS(r as u8, b as u8, g as u8, a as u8));
    }


    fn invert_region(&mut self, rect: &Rectangle) {}
    fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> Result<u32> {
        // print!("update {} {}", rect, mode);

        let rmRect = mxcfb_rect {
            top: rect.min.y as u32,
            left: rect.min.x as u32,
            width: rect.width(),
            height: rect.height()
        };

        let (is_partial, waveform_mode) = match mode {
            UpdateMode::Gui |
            UpdateMode::Partial  => (true, waveform_mode::WAVEFORM_MODE_AUTO),
            UpdateMode::Full     => (false, waveform_mode::WAVEFORM_MODE_GC16),
            UpdateMode::Fast |
            UpdateMode::FastMono => (true, waveform_mode::WAVEFORM_MODE_GLR16),
        };

        return if is_partial {
            Ok(self.fb.partial_refresh(
                &rmRect,
                PartialRefreshMode::Async,
                waveform_mode::WAVEFORM_MODE_DU,
                display_temp::TEMP_USE_REMARKABLE_DRAW,
                dither_mode::EPDC_FLAG_USE_DITHERING_PASSTHROUGH,
                0,
                false,
            ))
        } else {
            Ok(self.fb.full_refresh(
                waveform_mode::WAVEFORM_MODE_DU,
                display_temp::TEMP_USE_REMARKABLE_DRAW,
                dither_mode::EPDC_FLAG_USE_DITHERING_PASSTHROUGH,
                0,
                false))
        };
    }
    fn wait(&mut self, token: u32) -> Result<i32> {
        print!("wait {}\n", token);
        let res = self.fb.wait_refresh_complete(token) as i32;
        Ok(res)
    }
    fn save(&self, path: &str) -> Result<()> {
        print!("save {}", path);

        Ok(())
    }
    fn toggle_inverted(&mut self) {
        print!("toggle_inverted");
    }
    fn toggle_monochrome(&mut self) {
        print!("toggle_monochrome");
    }

    fn width(&self) -> u32 {
        self.fb.var_screen_info.xres
    }

    fn height(&self) -> u32 {
        self.fb.var_screen_info.yres
    }

}

impl<'a> RemarkableFramebuffer <'a> {
    pub fn new()  -> Result<RemarkableFramebuffer<'static>>  {
        let framebuffer = remarkable_fb::core::Framebuffer::new("/dev/fb0");
        Ok(RemarkableFramebuffer {
             fb: framebuffer
        })
    }
}