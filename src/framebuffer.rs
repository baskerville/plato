extern crate libc;
extern crate png;

use std::ptr;
use std::mem;
use std::path::Path;
use std::io;
use std::fs::{OpenOptions, File};
use std::slice;
use std::borrow::Cow;
use std::os::unix::io::AsRawFd;
use std::ops::Drop;
use libc::ioctl;
use png::HasParameters;
use geom::{Point, Rectangle, CornerSpec, surface_area};

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;

// Platform dependent
const MXCFB_SEND_UPDATE: libc::c_ulong = 0x4044462E;
const MXCFB_WAIT_FOR_UPDATE_COMPLETE: libc::c_ulong = 0x4004462F;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct FixScreenInfo {
    pub id: [u8; 16],
    pub smem_start: usize,
    pub smem_len: u32,
    pub kind: u32,
    pub type_aux: u32,
    pub visual: u32,
    pub xpanstep: u16,
    pub ypanstep: u16,
    pub ywrapstep: u16,
    pub line_length: u32,
    pub mmio_start: usize,
    pub mmio_len: u32,
    pub accel: u32,
    pub capabilities: u16,
    pub reserved: [u16; 2],
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct VarScreenInfo {
    pub xres: u32,
    pub yres: u32,
    pub xres_virtual: u32,
    pub yres_virtual: u32,
    pub xoffset: u32,
    pub yoffset: u32,
    pub bits_per_pixel: u32,
    pub grayscale: u32,
    pub red: Bitfield,
    pub green: Bitfield,
    pub blue: Bitfield,
    pub transp: Bitfield,
    pub nonstd: u32,
    pub activate: u32,
    pub height: u32,
    pub width: u32,
    pub accel_flags: u32,
    pub pixclock: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub upper_margin: u32,
    pub lower_margin: u32,
    pub hsync_len: u32,
    pub vsync_len: u32,
    pub sync: u32,
    pub vmode: u32,
    pub rotate: u32,
    pub colorspace: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct Bitfield {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

impl Default for Bitfield {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

impl Default for VarScreenInfo {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

impl Default for FixScreenInfo {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbRect {
    pub top: u32,
    pub left: u32,
    pub width: u32,
    pub height: u32,
}

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

#[repr(C)]
#[derive(Clone, Debug)]
struct MxcfbAltBufferData {
    virt_addr: *const libc::c_void,
    phys_addr: u32,
    width: u32,
    height: u32,
    alt_update_region: MxcfbRect,
}

#[repr(C)]
#[derive(Clone, Debug)]
struct MxcfbUpdateData {
    update_region: MxcfbRect,
    waveform_mode: u32,
    update_mode: u32,
    update_marker: u32,
    temp: libc::c_int,
    flags: libc::c_uint,
    alt_buffer_data: MxcfbAltBufferData,
}

const WAVEFORM_MODE_AUTO: u32 = 0x101; 

const NTX_WFM_MODE_INIT: u32  = 0;
const NTX_WFM_MODE_DU: u32    = 1;
const NTX_WFM_MODE_GC16: u32  = 2;
const NTX_WFM_MODE_GC4: u32   = 3;
const NTX_WFM_MODE_A2: u32    = 4;
const NTX_WFM_MODE_GL16: u32  = 5;
const NTX_WFM_MODE_GLR16: u32 = 6;
const NTX_WFM_MODE_GLD16: u32 = 7;

const UPDATE_MODE_PARTIAL: u32 = 0x0;
const UPDATE_MODE_FULL: u32    = 0x1;

const TEMP_USE_AMBIENT: libc::c_int = 0x1000;

const EPDC_FLAG_ENABLE_INVERSION: libc::c_uint = 0x01;
const EPDC_FLAG_FORCE_MONOCHROME: libc::c_uint = 0x02;

type SetPixelRgb = fn(&mut Framebuffer, u32, u32, [u8; 3]);
type GetPixelRgb = fn(&Framebuffer, u32, u32) -> [u8; 3];
type AsRgb = fn(&Framebuffer) -> Vec<u8>;

pub struct Framebuffer {
    device: File,
    frame: *mut libc::c_void,
    frame_size: libc::size_t, 
    token: u32,
    flags: u32,
    set_pixel_rgb: SetPixelRgb,
    get_pixel_rgb: GetPixelRgb,
    as_rgb: AsRgb,
    pub bytes_per_pixel: u8,
    pub var_info: VarScreenInfo,
    pub fix_info: FixScreenInfo,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum UpdateMode {
    Fast,
    Partial,
    Gui,
    Full,
}

#[derive(Debug, Clone)]
pub struct Bitmap {
    pub buf: Vec<u8>,
    pub width: i32,
    pub height: i32,
}

impl Framebuffer {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Framebuffer> {
        let device = OpenOptions::new().read(true)
                                       .write(true).open(path)?;

        let var_info = var_screen_info(&device)?;
        let fix_info = fix_screen_info(&device)?;

        assert_eq!(var_info.bits_per_pixel % 8, 0);

        let bytes_per_pixel = var_info.bits_per_pixel / 8;

        let mut frame_size = (var_info.xres_virtual *
                              var_info.yres_virtual * bytes_per_pixel) as libc::size_t;

        if frame_size > fix_info.smem_len as usize {
            frame_size = fix_info.smem_len as usize;
        }

        assert!(frame_size as u32 >= var_info.yres * fix_info.line_length);

        let frame = unsafe {
            libc::mmap(ptr::null_mut(), frame_size,
                       libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED,
                       device.as_raw_fd(), 0)
        };

        if frame == libc::MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            let (set_pixel_rgb, get_pixel_rgb, as_rgb): (SetPixelRgb, GetPixelRgb, AsRgb) = if var_info.bits_per_pixel > 16 {
                (set_pixel_rgb_32, get_pixel_rgb_32, as_rgb_32)
            } else {
                (set_pixel_rgb_16, get_pixel_rgb_16, as_rgb_16)
            };
            Ok(Framebuffer {
                   device: device,
                   frame: frame,
                   frame_size: frame_size,
                   token: 1,
                   flags: 0,
                   set_pixel_rgb: set_pixel_rgb,
                   get_pixel_rgb: get_pixel_rgb,
                   as_rgb: as_rgb,
                   bytes_per_pixel: bytes_per_pixel as u8,
                   var_info: var_info,
                   fix_info: fix_info,
               })
        }
    }
    
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        (self.set_pixel_rgb)(self, x, y, [color, color, color]);
    }

    pub fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        let rgb = (self.get_pixel_rgb)(self, x, y);
        let color_alpha = color as f32 * alpha;
        let r = color_alpha + (1.0 - alpha) * rgb[0] as f32;
        let g = color_alpha + (1.0 - alpha) * rgb[1] as f32;
        let b = color_alpha + (1.0 - alpha) * rgb[2] as f32;
        (self.set_pixel_rgb)(self, x, y, [r as u8, g as u8, b as u8]);
    }

    // Tell the driver that the screen needs to be redrawn.
    // The `rect` parameter is ignored for the `Gui` and `Full` modes.
    // The `Fast` mode only understands the following colors: BLACK and WHITE.
    pub fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> io::Result<u32> {
        let (update_mode, waveform_mode) = match mode {
            UpdateMode::Fast    => (UPDATE_MODE_PARTIAL, NTX_WFM_MODE_A2),
            UpdateMode::Partial => (UPDATE_MODE_PARTIAL, WAVEFORM_MODE_AUTO),
            UpdateMode::Gui     => (UPDATE_MODE_FULL, WAVEFORM_MODE_AUTO),
            UpdateMode::Full    => (UPDATE_MODE_FULL, NTX_WFM_MODE_GC16),
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
        let update_data = MxcfbUpdateData {
            update_region: (*rect).into(),
            waveform_mode: waveform_mode,
            update_mode: update_mode,
            update_marker: update_marker,
            temp: TEMP_USE_AMBIENT,
            flags: self.flags,
            alt_buffer_data: alt_buffer_data,
        };
        let result = unsafe {
            libc::ioctl(self.device.as_raw_fd(), MXCFB_SEND_UPDATE, &update_data)
        };
        match result {
            -1 => Err(io::Error::last_os_error()),
            _ => {
                self.token = self.token.wrapping_add(1);
                Ok(update_marker)
            }
        }
    }

    // Wait for a specific update to complete
    pub fn wait(&self, token: u32) -> io::Result<i32> {
        let result = unsafe {
            libc::ioctl(self.device.as_raw_fd(), MXCFB_WAIT_FOR_UPDATE_COMPLETE, &token)
        };
        match result {
            -1 => Err(io::Error::last_os_error()),
            _ => {
                Ok(result as i32)
            }
        }
    }

    pub fn draw_rectangle(&mut self, rect: &Rectangle, color: u8) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                self.set_pixel(x as u32, y as u32, color);
            }
        }
    }

    pub fn draw_blended_bitmap(&mut self, bitmap: &Bitmap, pt: &Point, color: u8) {
        for y in 0..bitmap.height {
            for x in 0..bitmap.width {
                let px = x + pt.x;
                let py = y + pt.y;
                let addr = (y * bitmap.width + x) as usize;
                let alpha = (255.0 - bitmap.buf[addr] as f32) / 255.0;
                self.set_blended_pixel(px as u32, py as u32, color, alpha);
            }
        }
    }

    pub fn draw_bitmap(&mut self, bitmap: &Bitmap, pt: &Point) {
        for y in 0..bitmap.height {
            for x in 0..bitmap.width {
                let px = x + pt.x;
                let py = y + pt.y;
                let addr = (y * bitmap.width + x) as usize;
                let color = bitmap.buf[addr];
                self.set_pixel(px as u32, py as u32, color);
            }
        }
    }

    pub fn clear(&mut self, color: u8) {
        let rect = self.rect();
        self.draw_rectangle(&rect, color);
    }

    pub fn draw_rounded_rectangle(&mut self, rect: &Rectangle, corners: &CornerSpec, color: u8) {
        let (nw, ne, se, sw) = match *corners {
            CornerSpec::Uniform(v) => (v, v, v, v),
            CornerSpec::North(v) => (v, v, 0, 0),
            CornerSpec::East(v) => (0, v, v, 0),
            CornerSpec::South(v) => (0, 0, v, v),
            CornerSpec::West(v) => (v, 0, 0, v),
            CornerSpec::Detailed {
                north_west,
                north_east,
                south_east,
                south_west
            } => (north_west, north_east, south_east, south_west),
        };
        let nw_c = rect.min + nw;
        let ne_c = pt!(rect.max.x - ne, rect.min.y + ne);
        let se_c = rect.max - se;
        let sw_c = pt!(rect.min.x + sw, rect.max.y - sw);
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let mut area = 1.0;
                let mut pole = None;
                if x < nw_c.x && y < nw_c.y {
                    pole = Some((nw_c, nw));
                } else if x >= ne_c.x && y < ne_c.y {
                    pole = Some((ne_c, ne));
                } else if x >= se_c.x && y >= se_c.y {
                    pole = Some((se_c, se));
                } else if x < sw_c.x && y >= sw_c.y {
                    pole = Some((sw_c, sw));
                }
                if let Some((center, radius)) = pole {
                    let v = vec2!((x - center.x) as f32, (y - center.y) as f32) + 0.5;
                    let angle = v.angle();
                    let dist = v.length() - radius as f32;
                    area = surface_area(dist, angle);
                }
                self.set_pixel(x as u32, y as u32, (area * color as f32) as u8);
            }
        }
    }

    pub fn draw_disk(&mut self, center: &Point, radius: i32, color: u8) {
        let rect = Rectangle::from_disk(center, radius);

        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let pt = Point::new(x, y);
                let v = vec2!((x - center.x) as f32, (y - center.y) as f32);
                let angle = v.angle();
                let dist = v.length() - radius as f32;
                let area = surface_area(dist, angle);
                self.set_pixel(x as u32, y as u32, (area * color as f32) as u8);
            }
        }
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.frame as *const u8, self.frame_size) }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) {
        let (width, height) = self.dims();
        let file = File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set(png::ColorType::RGB).set(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&(self.as_rgb)(self)).unwrap();
    }

    pub fn toggle_inverse(&mut self) {
        self.flags ^= EPDC_FLAG_ENABLE_INVERSION;
    }

    pub fn toggle_monochrome(&mut self) {
        self.flags ^= EPDC_FLAG_FORCE_MONOCHROME;
    }

    pub fn id(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.fix_info.id)
    }

    pub fn length(&self) -> usize {
        self.frame_size as usize
    }

    pub fn width(&self) -> u32 {
        self.var_info.xres
    }

    pub fn height(&self) -> u32 {
        self.var_info.yres
    }

    pub fn dims(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    pub fn rect(&self) -> Rectangle {
        let (width, height) = self.dims();
        rect![0, 0, width as i32, height as i32]
    }
}

#[inline]
pub fn set_pixel_rgb_16(fb: &mut Framebuffer, x: u32, y: u32, rgb: [u8; 3]) {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);

    debug_assert!(addr < fb.frame_size as isize);

    unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        *spot.offset(0) = rgb[2] >> 3 | (rgb[1] & 0b00011100) << 3;
        *spot.offset(1) = (rgb[0] & 0b11111000) | rgb[1] >> 5;
    }
}

#[inline]
pub fn set_pixel_rgb_32(fb: &mut Framebuffer, x: u32, y: u32, rgb: [u8; 3]) {
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

fn get_pixel_rgb_16(fb: &Framebuffer, x: u32, y: u32) -> [u8; 3] {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);
    let pair = unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        [*spot.offset(0), *spot.offset(1)]
    };
    let r = pair[1] & 0b11111000;
    let g = ((pair[1] & 0b00000111) << 5) | ((pair[0] & 0b11100000) >> 3);
    let b = (pair[0] & 0b00011111) << 3;
    [r, g, b]
}

fn get_pixel_rgb_32(fb: &Framebuffer, x: u32, y: u32) -> [u8; 3] {
    let addr = (fb.var_info.xoffset as isize + x as isize) * (fb.bytes_per_pixel as isize) +
               (fb.var_info.yoffset as isize + y as isize) * (fb.fix_info.line_length as isize);
    unsafe {
        let spot = fb.frame.offset(addr) as *mut u8;
        [*spot.offset(2), *spot.offset(1), *spot.offset(0)]
    }
}

fn as_rgb_16(fb: &Framebuffer) -> Vec<u8> {
    let (width, height) = fb.dims();
    let mut rgb888 = Vec::with_capacity((width * height * 3) as usize);
    let rgb565 = fb.as_bytes();
    let virtual_width = fb.var_info.xres_virtual as usize;
    for (_, pair) in rgb565.chunks(2).take(height as usize * virtual_width).enumerate()
                           .filter(|&(i, _)| i % virtual_width < width as usize) {
        let r = pair[1] & 0b11111000;
        let g = ((pair[1] & 0b00000111) << 5) | ((pair[0] & 0b11100000) >> 3);
        let b = (pair[0] & 0b00011111) << 3;
        rgb888.extend_from_slice(&[r, g, b]);
    }
    rgb888
}

fn as_rgb_32(fb: &Framebuffer) -> Vec<u8> {
    let (width, height) = fb.dims();
    let mut rgb888 = Vec::with_capacity((width * height * 3) as usize);
    let bgra8888 = fb.as_bytes();
    let virtual_width = fb.var_info.xres_virtual as usize;
    for (_, bgra) in bgra8888.chunks(4).take(height as usize * virtual_width).enumerate()
                           .filter(|&(i, _)| i % virtual_width < width as usize) {
        let r = bgra[2];
        let g = bgra[1];
        let b = bgra[0];
        rgb888.extend_from_slice(&[r, g, b]);
    }
    rgb888
}

pub fn fix_screen_info(device: &File) -> io::Result<FixScreenInfo> {
    let mut info: FixScreenInfo = Default::default();
    let result = unsafe { ioctl(device.as_raw_fd(), FBIOGET_FSCREENINFO, &mut info) };
    match result {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(info),
    }
}

pub fn var_screen_info(device: &File) -> io::Result<VarScreenInfo> {
    let mut info: VarScreenInfo = Default::default();
    let result = unsafe { ioctl(device.as_raw_fd(), FBIOGET_VSCREENINFO, &mut info) };
    match result {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(info),
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.frame, self.frame_size);
        }
    }
}
