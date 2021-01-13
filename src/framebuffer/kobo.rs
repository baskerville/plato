use std::ptr;
use std::path::Path;
use std::io;
use std::fs::{OpenOptions, File};
use std::slice;
use std::os::unix::io::AsRawFd;
use std::ops::Drop;
use anyhow::{Error, Context};
use lazy_static::lazy_static;
use crate::geom::Rectangle;
use crate::device::{CURRENT_DEVICE, Model};
use super::{UpdateMode, Framebuffer};
use super::image::Pixmap;
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

type ColorTransform = fn(u32, u32, u8) -> u8;
type SetPixelRgb = fn(&mut KoboFramebuffer, u32, u32, [u8; 3]);
type GetPixelRgb = fn(&KoboFramebuffer, u32, u32) -> [u8; 3];
type AsRgb = fn(&KoboFramebuffer) -> Vec<u8>;

pub struct KoboFramebuffer {
    file: File,
    frame: *mut libc::c_void,
    frame_size: libc::size_t, 
    token: u32,
    flags: u32,
    monochrome: bool,
    dithered: bool,
    transform: ColorTransform,
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
                                     .open(&path)
                                     .with_context(|| format!("Can't open framebuffer device {}.", path.as_ref().display()))?;

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
            Err(Error::from(io::Error::last_os_error()).context("Can't map memory."))
        } else {
            let (set_pixel_rgb, get_pixel_rgb, as_rgb): (SetPixelRgb, GetPixelRgb, AsRgb) = if var_info.bits_per_pixel > 16 {
                (set_pixel_rgb_32, get_pixel_rgb_32, as_rgb_32)
            } else if var_info.bits_per_pixel > 8 {
                (set_pixel_rgb_16, get_pixel_rgb_16, as_rgb_16)
            } else {
                (set_pixel_rgb_8, get_pixel_rgb_8, as_rgb_8)
            };
            Ok(KoboFramebuffer {
                   file,
                   frame,
                   frame_size,
                   token: 1,
                   flags: 0,
                   monochrome: false,
                   dithered: false,
                   transform: transform_identity,
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
        let c = (self.transform)(x, y, color);
        (self.set_pixel_rgb)(self, x, y, [c, c, c]);
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        if alpha >= 1.0 {
            self.set_pixel(x, y, color);
            return;
        }
        let rgb = (self.get_pixel_rgb)(self, x, y);
        let color_alpha = color as f32 * alpha;
        let interp = (color_alpha + (1.0 - alpha) * rgb[0] as f32) as u8;
        let c = (self.transform)(x, y, interp);
        (self.set_pixel_rgb)(self, x, y, [c, c, c]);
    }

    fn invert_region(&mut self, rect: &Rectangle) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let rgb = (self.get_pixel_rgb)(self, x as u32, y as u32);
                let color = 255 - rgb[0];
                (self.set_pixel_rgb)(self, x as u32, y as u32, [color, color, color]);
            }
        }
    }

    fn shift_region(&mut self, rect: &Rectangle, drift: u8) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let rgb = (self.get_pixel_rgb)(self, x as u32, y as u32);
                let color = rgb[0].saturating_sub(drift);
                (self.set_pixel_rgb)(self, x as u32, y as u32, [color, color, color]);
            }
        }
    }

    // Tell the driver that the screen needs to be redrawn.
    fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> Result<u32, Error> {
        let update_marker = self.token;
        let mark = CURRENT_DEVICE.mark();
        let mut flags = self.flags;
        let mut monochrome = self.monochrome;

        let (update_mode, mut waveform_mode) = match mode {
            UpdateMode::Gui => (UPDATE_MODE_PARTIAL, NTX_WFM_MODE_GL16),
            UpdateMode::Partial => {
                if mark >= 7 {
                    (UPDATE_MODE_PARTIAL, NTX_WFM_MODE_GLR16)
                } else if CURRENT_DEVICE.model == Model::Aura {
                    flags |= EPDC_FLAG_USE_AAD;
                    (UPDATE_MODE_FULL, NTX_WFM_MODE_GLD16)
                } else {
                    (UPDATE_MODE_PARTIAL, NTX_WFM_MODE_GL16)
                }
            },
            UpdateMode::Full => {
                monochrome = false;
                (UPDATE_MODE_FULL, NTX_WFM_MODE_GC16)
            },
            UpdateMode::Fast => (UPDATE_MODE_PARTIAL, NTX_WFM_MODE_A2),
            UpdateMode::FastMono => {
                flags |= EPDC_FLAG_FORCE_MONOCHROME;
                (UPDATE_MODE_PARTIAL, NTX_WFM_MODE_A2)
            },
        };

        if monochrome {
            if mark >= 7 {
                if waveform_mode != NTX_WFM_MODE_A2 {
                    waveform_mode = NTX_WFM_MODE_DU;
                    if !self.dithered {
                        flags |= EPDC_FLAG_USE_DITHERING_Y1;
                    }
                }
            } else {
                waveform_mode = NTX_WFM_MODE_A2;
            }
        }

        let result = if mark >= 7 {
            let mut quant_bit = 0;
            let mut dither_mode = EPDC_FLAG_USE_DITHERING_PASSTHROUGH;

            if self.dithered {
                if monochrome {
                    flags |= EPDC_FLAG_USE_DITHERING_Y1;
                } else if mode == UpdateMode::Partial || mode == UpdateMode::Full {
                    dither_mode = EPDC_FLAG_USE_DITHERING_ORDERED;
                    quant_bit = 7;
                }
            }

            let update_data = MxcfbUpdateDataV2 {
                update_region: (*rect).into(),
                waveform_mode,
                update_mode,
                update_marker,
                temp: TEMP_USE_AMBIENT,
                flags,
                dither_mode,
                quant_bit,
                alt_buffer_data: MxcfbAltBufferDataV2::default(),
            };
            unsafe {
                send_update_v2(self.file.as_raw_fd(), &update_data)
            }
        } else {
            if monochrome && !self.dithered {
                flags |= EPDC_FLAG_FORCE_MONOCHROME;
            }

            let update_data = MxcfbUpdateDataV1 {
                update_region: (*rect).into(),
                waveform_mode,
                update_mode,
                update_marker,
                temp: TEMP_USE_AMBIENT,
                flags,
                alt_buffer_data: MxcfbAltBufferDataV1::default(),
            };
            unsafe {
                send_update_v1(self.file.as_raw_fd(), &update_data)
            }
        };

        match result {
            Err(e) => Err(Error::from(e).context("Can't send framebuffer update.")),
            _ => {
                self.token = self.token.wrapping_add(1);
                Ok(update_marker)
            }
        }
    }

    // Wait for a specific update to complete.
    fn wait(&self, token: u32) -> Result<i32, Error> {
        let result = if CURRENT_DEVICE.mark() >= 7 {
            let mut marker_data = MxcfbUpdateMarkerData {
                update_marker: token,
                collision_test: 0,
            };
            unsafe {
                wait_for_update_v2(self.file.as_raw_fd(), &mut marker_data)
            }
        } else {
            unsafe {
                wait_for_update_v1(self.file.as_raw_fd(), &token)
            }
        };
        result.context("Can't wait for framebuffer update.")
    }

    fn save(&self, path: &str) -> Result<(), Error> {
        let (width, height) = self.dims();
        let file = File::create(path).with_context(|| format!("Can't create output file {}.", path))?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_color(png::ColorType::RGB);
        let mut writer = encoder.write_header().with_context(|| format!("Can't write PNG header for {}.", path))?;
        writer.write_image_data(&(self.as_rgb)(self)).with_context(|| format!("Can't write PNG data to {}.", path))?;
        Ok(())
    }

    #[inline]
    fn rotation(&self) -> i8 {
        self.var_info.rotate as i8
    }

    fn set_rotation(&mut self, n: i8) -> Result<(u32, u32), Error> {
        let read_rotation = self.rotation();

        // On the Aura H₂O, the first ioctl call will succeed but have no effect,
        // if (n - m).abs() % 2 == 1, where m is the previously written value.
        // In order for the call to have an effect, we need to write an intermediate
        // value: (n+1)%4.
        for (i, v) in [n, (n+1)%4, n].iter().enumerate() {
            self.var_info.rotate = *v as u32;

            let result = unsafe {
                write_variable_screen_info(self.file.as_raw_fd(), &mut self.var_info)
            };

            if let Err(e) = result {
                return Err(Error::from(e)
                                 .context("Can't set variable screen info."));
            }

            // If the first call changed the rotation value, we can exit the loop.
            if i == 0 && read_rotation != self.rotation() {
                break;
            }
        }

        self.fix_info = fix_screen_info(&self.file)?;
        self.frame_size = (self.var_info.yres * self.fix_info.line_length) as libc::size_t;

        println!("Framebuffer rotation: {} -> {}.", n, self.rotation());

        Ok((self.var_info.xres, self.var_info.yres))
    }

    fn set_inverted(&mut self, enable: bool) {
        if enable {
            self.flags |= EPDC_FLAG_ENABLE_INVERSION;
        } else {
            self.flags &= !EPDC_FLAG_ENABLE_INVERSION;
        }
    }

    fn inverted(&self) -> bool {
        self.flags & EPDC_FLAG_ENABLE_INVERSION != 0
    }

    fn set_monochrome(&mut self, enable: bool) {
        self.monochrome = enable;
    }

    fn monochrome(&self) -> bool {
        self.monochrome
    }

    fn set_dithered(&mut self, enable: bool) {
        if enable == self.dithered {
            return;
        }

        self.dithered = enable;

        if CURRENT_DEVICE.mark() < 7 {
            if enable {
                self.transform = transform_dither_g16;
            } else {
                self.transform = transform_identity;
            }
        }
    }

    fn dithered(&self) -> bool {
        self.dithered
    }

    fn width(&self) -> u32 {
        self.var_info.xres
    }

    fn height(&self) -> u32 {
        self.var_info.yres
    }
}

fn set_pixel_rgb_8(fb: &mut KoboFramebuffer, x: u32, y: u32, rgb: [u8; 3]) {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);

    debug_assert!(addr < fb.frame_size as isize);

    unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        *spot = rgb[0];
    }
}

fn set_pixel_rgb_16(fb: &mut KoboFramebuffer, x: u32, y: u32, rgb: [u8; 3]) {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);

    debug_assert!(addr < fb.frame_size as isize);

    unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        *spot.offset(0) = rgb[2] >> 3 | (rgb[1] & 0b0001_1100) << 3;
        *spot.offset(1) = (rgb[0] & 0b1111_1000) | rgb[1] >> 5;
    }
}

fn set_pixel_rgb_32(fb: &mut KoboFramebuffer, x: u32, y: u32, rgb: [u8; 3]) {
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

fn get_pixel_rgb_8(fb: &KoboFramebuffer, x: u32, y: u32) -> [u8; 3] {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);
    let gray = unsafe { *(fb.frame.offset(addr) as *const u8) };
    [gray, gray, gray]
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

fn as_rgb_8(fb: &KoboFramebuffer) -> Vec<u8> {
    let (width, height) = fb.dims();
    let mut rgb888 = Vec::with_capacity((width * height * 3) as usize);
    let rgb8 = fb.as_bytes();
    let virtual_width = fb.var_info.xres_virtual as usize;
    for (_, &gray) in rgb8.iter().take(height as usize * virtual_width).enumerate()
                          .filter(|&(i, _)| i % virtual_width < width as usize) {
        rgb888.extend_from_slice(&[gray, gray, gray]);
    }
    rgb888
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

const DITHER_PITCH: u32 = 128;

lazy_static! {
    pub static ref DITHER_G16_DRIFTS: Vec<i8> = {
        let pixmap = Pixmap::from_png("resources/blue_noise-128.png").unwrap();
        // The gap between two succesive colors in G16 is 17.
        // Map {0 .. 255} to {-9 .. 8}.
        pixmap.data().iter().map(|v| (*v / 15) as i8 - 9).collect()
    };
}

// The input color is in {0 .. 255}.
// The output color is in G16.
// G16 := {17 * i | i ∈ {0 .. 15}}.
fn transform_dither_g16(x: u32, y: u32, color: u8) -> u8 {
    // Get the address of the drift value.
    let addr = (x % DITHER_PITCH) + (y % DITHER_PITCH) * DITHER_PITCH;
    // Apply the drift to the input color.
    let c = (color as i16 + DITHER_G16_DRIFTS[addr as usize] as i16).max(0).min(255);
    // Compute the distance to the previous color in G16.
    let d = c % 17;
    // Return the nearest color in G16.
    if d < 9 {
        (c - d) as u8
    } else {
        (c + (17 - d)) as u8
    }
}

fn transform_identity(_x: u32, _y: u32, color: u8) -> u8 {
    color
}

fn fix_screen_info(file: &File) -> Result<FixScreenInfo, Error> {
    let mut info: FixScreenInfo = Default::default();
    let result = unsafe {
        read_fixed_screen_info(file.as_raw_fd(), &mut info)
    };
    match result {
        Err(e) => Err(Error::from(e).context("Can't get fixed screen info.")),
        _ => Ok(info),
    }
}

fn var_screen_info(file: &File) -> Result<VarScreenInfo, Error> {
    let mut info: VarScreenInfo = Default::default();
    let result = unsafe {
        read_variable_screen_info(file.as_raw_fd(), &mut info)
    };
    match result {
        Err(e) => Err(Error::from(e).context("Can't get variable screen info.")),
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
