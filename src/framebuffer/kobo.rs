extern crate libc;
extern crate png;

use std::ptr;
use std::path::Path;
use std::io;
use std::fs::{OpenOptions, File};
use std::slice;
use std::os::unix::io::AsRawFd;
use std::ops::Drop;
use failure::{Error, ResultExt};
use libc::ioctl;
use png::HasParameters;
use geom::Rectangle;
use device::{Model, CURRENT_DEVICE};
use super::{UpdateMode, Framebuffer};
use super::mxcfb_sys::*;

impl Into<MxcfbRect> for Rectangle {
    fn into(self) -> MxcfbRect {
        MxcfbRect {
            top: self.min.y as u32,
            left: self.min.x as u32,
            width: self.width(),
            height: self.height(),
        }
    }
}

type SetPixelRgb = fn(&mut KoboFramebuffer, u32, u32, [u8; 3]);
type GetPixelRgb = fn(&KoboFramebuffer, u32, u32) -> [u8; 3];
type AsRgb = fn(&KoboFramebuffer) -> Vec<u8>;

pub struct KoboFramebuffer {
    file: File,
    frame: *mut libc::c_void,
    frame_size: libc::size_t, 
    token: u32,
    flags: u32,
    set_pixel_rgb: SetPixelRgb,
    get_pixel_rgb: GetPixelRgb,
    as_rgb: AsRgb,
    bytes_per_pixel: u8,
    var_info: VarScreenInfo,
    fix_info: FixScreenInfo,
}

impl KoboFramebuffer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<KoboFramebuffer, Error> {
        let file = OpenOptions::new().read(true)
                                     .write(true)
                                     .open(path)
                                     .context("Can't open framebuffer device.")?;

        let var_info = var_screen_info(&file)?;
        let fix_info = fix_screen_info(&file)?;

        assert_eq!(var_info.bits_per_pixel % 8, 0);

        let bytes_per_pixel = var_info.bits_per_pixel / 8;
        let frame_size = (var_info.yres * fix_info.line_length) as libc::size_t;

        let frame = unsafe {
            libc::mmap(ptr::null_mut(), fix_info.smem_len as usize,
                       libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED,
                       file.as_raw_fd(), 0)
        };

        if frame == libc::MAP_FAILED {
            Err(Error::from(io::Error::last_os_error()).context("Can't map memory.").into())
        } else {
            let (set_pixel_rgb, get_pixel_rgb, as_rgb): (SetPixelRgb, GetPixelRgb, AsRgb) = if var_info.bits_per_pixel > 16 {
                (set_pixel_rgb_32, get_pixel_rgb_32, as_rgb_32)
            } else {
                (set_pixel_rgb_16, get_pixel_rgb_16, as_rgb_16)
            };
            Ok(KoboFramebuffer {
                   file,
                   frame,
                   frame_size,
                   token: 1,
                   flags: 0,
                   set_pixel_rgb,
                   get_pixel_rgb,
                   as_rgb,
                   bytes_per_pixel: bytes_per_pixel as u8,
                   var_info,
                   fix_info,
               })
        }
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.frame as *const u8, self.frame_size) }
    }
}

impl Framebuffer for KoboFramebuffer {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        (self.set_pixel_rgb)(self, x, y, [color, color, color]);
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        if alpha >= 1.0 {
            self.set_pixel(x, y, color);
            return;
        }
        let rgb = (self.get_pixel_rgb)(self, x, y);
        let color_alpha = color as f32 * alpha;
        let red = color_alpha + (1.0 - alpha) * rgb[0] as f32;
        let green = color_alpha + (1.0 - alpha) * rgb[1] as f32;
        let blue = color_alpha + (1.0 - alpha) * rgb[2] as f32;
        (self.set_pixel_rgb)(self, x, y, [red as u8, green as u8, blue as u8]);
    }

    fn invert_region(&mut self, rect: &Rectangle) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let rgb = (self.get_pixel_rgb)(self, x as u32, y as u32);
                let red = 255 - rgb[0];
                let green = 255 - rgb[1];
                let blue = 255 - rgb[2];
                (self.set_pixel_rgb)(self, x as u32, y as u32, [red, green, blue]);
            }
        }
    }

    // Tell the driver that the screen needs to be redrawn.
    // The `rect` parameter is ignored for the `Full` mode.
    // The `Fast` mode maps everything to BLACK and WHITE.
    fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> Result<u32, Error> {
        let (update_mode, waveform_mode) = match mode {
            UpdateMode::Gui |
            UpdateMode::Partial  => (UPDATE_MODE_PARTIAL, WAVEFORM_MODE_AUTO),
            UpdateMode::Full     => (UPDATE_MODE_FULL, NTX_WFM_MODE_GC16),
            UpdateMode::Fast |
            UpdateMode::FastMono => (UPDATE_MODE_PARTIAL, NTX_WFM_MODE_A2),
        };
        let alt_buffer_data = MxcfbAltBufferData {
            virt_addr: ptr::null(),
            phys_addr: 0,
            width: 0,
            height: 0,
            alt_update_region: MxcfbRect {
                top: 0,
                left: 0,
                width: 0,
                height: 0,
            },
        };
        let update_marker = self.token;
        let mut flags = self.flags;
        if mode == UpdateMode::FastMono {
            flags |= EPDC_FLAG_FORCE_MONOCHROME;
        }
        let update_data = MxcfbUpdateData {
            update_region: (*rect).into(),
            waveform_mode,
            update_mode,
            update_marker,
            temp: TEMP_USE_AMBIENT,
            flags,
            alt_buffer_data,
        };
        let result = unsafe {
            libc::ioctl(self.file.as_raw_fd(), MXCFB_SEND_UPDATE, &update_data)
        };
        match result {
            -1 => Err(Error::from(io::Error::last_os_error()).context("Can't update framebuffer.").into()),
            _ => {
                self.token = self.token.wrapping_add(1);
                Ok(update_marker)
            }
        }
    }

    // Wait for a specific update to complete
    fn wait(&self, token: u32) -> Result<i32, Error> {
        let result = unsafe {
            libc::ioctl(self.file.as_raw_fd(), MXCFB_WAIT_FOR_UPDATE_COMPLETE, &token)
        };
        match result {
            -1 => Err(Error::from(io::Error::last_os_error()).context("Can't wait for framebuffer update.").into()),
            _ => {
                Ok(result as i32)
            }
        }
    }

    fn save(&self, path: &str) -> Result<(), Error> {
        let (width, height) = self.dims();
        let file = File::create(path).context("Can't create output file.")?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set(png::ColorType::RGB).set(png::BitDepth::Eight);
        let mut writer = encoder.write_header().context("Can't write header.")?;
        writer.write_image_data(&(self.as_rgb)(self)).context("Can't write data to file.")?;
        Ok(())
    }

    fn rotation(&self) -> i8 {
        self.var_info.rotate as i8
    }

    fn set_rotation(&mut self, mut n: i8) -> Result<(u32, u32), Error> {
        match CURRENT_DEVICE.model {
            Model::AuraH2O | Model::AuraH2OEdition2 | Model::AuraHD => n ^= 2,
            _ => (),
        }
        self.var_info.rotate = n as u32;
        let result = unsafe {
            libc::ioctl(self.file.as_raw_fd(), FBIOPUT_VSCREENINFO, &mut self.var_info)
        };
        match result {
            -1 => Err(Error::from(io::Error::last_os_error())
                            .context("Can't set variable screen info.").into()),
            _ => {
                self.fix_info = fix_screen_info(&self.file)?;
                self.frame_size = (self.var_info.yres * self.fix_info.line_length) as libc::size_t;
                Ok((self.var_info.xres, self.var_info.yres))
            }
        }
    }


    fn toggle_inverted(&mut self) {
        self.flags ^= EPDC_FLAG_ENABLE_INVERSION;
    }

    fn toggle_monochrome(&mut self) {
        self.flags ^= EPDC_FLAG_FORCE_MONOCHROME;
    }

    fn width(&self) -> u32 {
        self.var_info.xres
    }

    fn height(&self) -> u32 {
        self.var_info.yres
    }
}

#[inline]
pub fn set_pixel_rgb_16(fb: &mut KoboFramebuffer, x: u32, y: u32, rgb: [u8; 3]) {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);

    debug_assert!(addr < fb.frame_size as isize);

    unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        *spot.offset(0) = rgb[2] >> 3 | (rgb[1] & 0b0001_1100) << 3;
        *spot.offset(1) = (rgb[0] & 0b1111_1000) | rgb[1] >> 5;
    }
}

#[inline]
pub fn set_pixel_rgb_32(fb: &mut KoboFramebuffer, x: u32, y: u32, rgb: [u8; 3]) {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);

    debug_assert!(addr < fb.frame_size as isize);

    unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        *spot.offset(0) = rgb[2];
        *spot.offset(1) = rgb[1];
        *spot.offset(2) = rgb[0];
        // *spot.offset(3) = 0x00;
    }
}

fn get_pixel_rgb_16(fb: &KoboFramebuffer, x: u32, y: u32) -> [u8; 3] {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);
    let pair = unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        [*spot.offset(0), *spot.offset(1)]
    };
    let red = pair[1] & 0b1111_1000;
    let green = ((pair[1] & 0b0000_0111) << 5) | ((pair[0] & 0b1110_0000) >> 3);
    let blue = (pair[0] & 0b0001_1111) << 3;
    [red, green, blue]
}

fn get_pixel_rgb_32(fb: &KoboFramebuffer, x: u32, y: u32) -> [u8; 3] {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);
    unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        [*spot.offset(2), *spot.offset(1), *spot.offset(0)]
    }
}

fn as_rgb_16(fb: &KoboFramebuffer) -> Vec<u8> {
    let (width, height) = fb.dims();
    let mut rgb888 = Vec::with_capacity((width * height * 3) as usize);
    let rgb565 = fb.as_bytes();
    let virtual_width = fb.var_info.xres_virtual as usize;
    for (_, pair) in rgb565.chunks(2).take(height as usize * virtual_width).enumerate()
                           .filter(|&(i, _)| i % virtual_width < width as usize) {
        let red = pair[1] & 0b1111_1000;
        let green = ((pair[1] & 0b0000_0111) << 5) | ((pair[0] & 0b1110_0000) >> 3);
        let blue = (pair[0] & 0b0001_1111) << 3;
        rgb888.extend_from_slice(&[red, green, blue]);
    }
    rgb888
}

fn as_rgb_32(fb: &KoboFramebuffer) -> Vec<u8> {
    let (width, height) = fb.dims();
    let mut rgb888 = Vec::with_capacity((width * height * 3) as usize);
    let bgra8888 = fb.as_bytes();
    let virtual_width = fb.var_info.xres_virtual as usize;
    for (_, bgra) in bgra8888.chunks(4).take(height as usize * virtual_width).enumerate()
                           .filter(|&(i, _)| i % virtual_width < width as usize) {
        let red = bgra[2];
        let green = bgra[1];
        let blue = bgra[0];
        rgb888.extend_from_slice(&[red, green, blue]);
    }
    rgb888
}

pub fn fix_screen_info(file: &File) -> Result<FixScreenInfo, Error> {
    let mut info: FixScreenInfo = Default::default();
    let result = unsafe { ioctl(file.as_raw_fd(), FBIOGET_FSCREENINFO, &mut info) };
    match result {
        -1 => Err(Error::from(io::Error::last_os_error()).context("Can't get fixed screen info.").into()),
        _ => Ok(info),
    }
}

pub fn var_screen_info(file: &File) -> Result<VarScreenInfo, Error> {
    let mut info: VarScreenInfo = Default::default();
    let result = unsafe { ioctl(file.as_raw_fd(), FBIOGET_VSCREENINFO, &mut info) };
    match result {
        -1 => Err(Error::from(io::Error::last_os_error()).context("Can't get variable screen info.").into()),
        _ => Ok(info),
    }
}

impl Drop for KoboFramebuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.frame, self.fix_info.smem_len as usize);
        }
    }
}
