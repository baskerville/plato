
extern crate libremarkable;
use libremarkable::fbdraw::FramebufferDraw;
use libremarkable::mxc_types::{VarScreeninfo,FixScreeninfo};

use framebuffer::{UpdateMode, Framebuffer};


pub struct RemarkableFramebuffer {
	fb: libremarkable::fb::Framebuffer
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
    // fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32);
    // fn invert_region(&mut self, rect: &Rectangle);
    fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> Result<u32> {
    	fb.refresh(rmRect, rmUpdateMode, rmWaveformMode, tempMode, ditherMode, )
    }
    fn wait(&self, token: u32) -> Result<i32> {
    	fb.wait_refresh_complete(token)
    }
    // fn save(&self, path: &str) -> Result<()>;
    // fn toggle_inverted(&mut self);
    // fn toggle_monochrome(&mut self);

}

impl RemarkableFramebuffer {
    pub fn new() -> Result<RemarkableFramebuffer> {
    	let framebuffer = Box::new(libremarkable::fb::Framebuffer::new("/dev/fb0"));
        // let yres = framebuffer.var_screen_info.yres;
        // let xres = framebuffer.var_screen_info.xres;
        Ok(RemarkableFramebuffer { framebuffer })
    }
}