use std::io;
use std::ptr;
use std::mem;
use std::slice;
use std::path::Path;
use std::mem::ManuallyDrop;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::ops::Drop;
use anyhow::{Error, Context};
use crate::geom::Rectangle;
use crate::device::CURRENT_DEVICE;
use super::{UpdateMode, Framebuffer};
use super::linuxfb_sys::*;
use super::ion_sys::*;
use super::sunxi_sys::*;
use super::transform::*;

impl From<Rectangle> for AreaInfo {
    fn from(rect: Rectangle) -> Self {
        AreaInfo {
            x_top: rect.min.x as libc::c_uint,
            y_top: rect.min.y as libc::c_uint,
            x_bottom: (rect.max.x - 1) as libc::c_uint,
            y_bottom: (rect.max.y - 1) as libc::c_uint,
        }
    }
}

pub struct KoboFramebuffer2 {
    ion: File,
    display: File,
    fd_data: IonFdData,
    layer: DispLayerConfig2,
    frame: *mut libc::c_void,
    frame_size: usize,
    alloc_size: libc::size_t,
    var_info: VarScreenInfo,
    fix_info: FixScreenInfo,
    transform: ColorTransform,
    token: u32,
    monochrome: bool,
    inverted: bool,
    dithered: bool,
}

const MEM_ALIGN: u32 = 4096;

#[inline]
fn align(value: u32, align: u32) -> u32 {
    let mask = align - 1;
    (value + mask) & !mask
}

impl KoboFramebuffer2 {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<KoboFramebuffer2, Error> {
        let file = File::open(&path)
                        .with_context(|| format!("can't open framebuffer device {}", path.as_ref().display()))?;

        let mut var_info = var_screen_info(&file)?;
        let mut fix_info = fix_screen_info(&file)?;

        var_info.rotate = CURRENT_DEVICE.startup_rotation() as u32;

        if var_info.xres > var_info.yres {
            mem::swap(&mut var_info.xres, &mut var_info.yres);
        }

        var_info.xres_virtual = var_info.xres;
        var_info.yres_virtual = var_info.yres;
        var_info.bits_per_pixel = 8;
        var_info.grayscale = 1;
        fix_info.line_length = var_info.xres_virtual;
        fix_info.smem_len = fix_info.line_length * var_info.yres_virtual;

        let ion = File::open("/dev/ion")
                       .with_context(|| "can't open ion device")?;

        let alloc_size = align(fix_info.smem_len, MEM_ALIGN) as libc::size_t;

        let mut data = IonAllocationData {
            len: alloc_size,
            align: MEM_ALIGN as libc::size_t,
            heap_id_mask: ION_HEAP_MASK_CARVEOUT,
            flags: 0,
            handle: 0,
        };

        let result = unsafe {
            ion_alloc(ion.as_raw_fd(), &mut data)
        };

        if let Err(e) = result {
            return Err(Error::from(e).context("can't allocate memory"));
        }

        let mut data = IonFdData {
            handle: data.handle,
            fd: -1,
        };

        let result = unsafe {
            ion_map(ion.as_raw_fd(), &mut data)
        };

        if let Err(e) = result {
            let mut data = IonHandleData { handle: data.handle };
            let _ = unsafe { ion_free(ion.as_raw_fd(), &mut data) };
            return Err(Error::from(e).context("can't get mappable file descriptor"));
        }

        let frame = unsafe {
            libc::mmap(ptr::null_mut(), alloc_size,
                       libc::PROT_READ | libc::PROT_WRITE,
                       libc::MAP_SHARED,
                       data.fd, 0)
        };

        if frame == libc::MAP_FAILED {
            unsafe { libc::close(data.fd) };
            let mut data = IonHandleData { handle: data.handle };
            let _ = unsafe { ion_free(ion.as_raw_fd(), &mut data) };
            return Err(Error::from(io::Error::last_os_error()).context("can't map memory"));
        }

        let display = File::open("/dev/disp");

        if let Err(e) = display {
            let _ = unsafe { libc::munmap(frame, alloc_size) };
            unsafe { libc::close(data.fd) };
            let mut data = IonHandleData { handle: data.handle };
            let _ = unsafe { ion_free(ion.as_raw_fd(), &mut data) };
            return Err(Error::from(e).context("can't open display device"));
        }

        let frame_size = (var_info.yres * fix_info.line_length) as usize;

        let layer = DispLayerConfig2 {
            info: DispLayerInfo2 {
                mode: LAYER_MODE_BUFFER,
                zorder: 0,
                alpha_mode: 1,
                alpha_value: 0xFF,
                screen_win: DispRect {
                    x: 0,
                    y: 0,
                    width: var_info.xres,
                    height: var_info.yres,
                },
                b_trd_out: false,
                out_trd_mode: 0,
                color_fb: ColorFb {
                    fb: ManuallyDrop::new(
                        DispFbInfo2 {
                            fd: 0,
                            y8_fd: data.fd,
                            size: [
                                DispRectsz {
                                    width: var_info.xres_virtual,
                                    height: var_info.yres_virtual,
                                },
                                DispRectsz { width: 0, height: 0 },
                                DispRectsz { width: 0, height: 0 },
                            ],
                            align: [0; 3],
                            format: DISP_FORMAT_8BIT_GRAY,
                            color_space: DISP_GBR_F,
                            trd_right_fd: 0,
                            pre_multiply: true,
                            crop: DispRect64 {
                                x: 0,
                                y: 0,
                                width: (var_info.xres as libc::c_longlong) << 32,
                                height: (var_info.yres as libc::c_longlong) << 32,
                            },
                            flags: DISP_BF_NORMAL,
                            scan: DISP_SCAN_PROGRESSIVE,
                            eotf: DISP_EOTF_GAMMA22,
                            depth: 0,
                            fbd_en: 0,
                            metadata_fd: 0,
                            metadata_size: 0,
                            metadata_flag: 0,
                        },
                    ),
                },
                id: 0,
                atw: DispAtwInfo {
                    used: false,
                    mode: 0,
                    b_row: 0,
                    b_col: 0,
                    colf_fd: 0,
                },
            },
            enable: true,
            channel: 0,
            layer_id: 1,
        };

        Ok(KoboFramebuffer2 {
               ion,
               display: display.unwrap(),
               fd_data: data,
               layer,
               frame,
               frame_size,
               alloc_size,
               token: 1,
               monochrome: false,
               inverted: false,
               dithered: false,
               transform: transform_identity,
               var_info,
               fix_info,
           })
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.frame as *const u8, self.frame_size) }
    }

    fn get_pixel(&self, x: u32, y: u32) -> u8 {
        let addr = (x + y * self.fix_info.line_length) as isize;
        let c = unsafe { *(self.frame.offset(addr) as *const u8) };
        if self.inverted {
            255 - c
        } else {
            c
        }
    }
}

impl Framebuffer for KoboFramebuffer2 {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        let mut c = (self.transform)(x, y, color);
        if self.inverted {
            c = 255 - c;
        }
        let addr = (x + y * self.fix_info.line_length) as isize;
        let spot = unsafe { self.frame.offset(addr) as *mut u8 };
        unsafe { *spot = c };
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        if alpha >= 1.0 {
            self.set_pixel(x, y, color);
            return;
        }
        let cur = self.get_pixel(x, y);
        let color_alpha = color as f32 * alpha;
        let interp = (color_alpha + (1.0 - alpha) * cur as f32) as u8;
        let c = (self.transform)(x, y, interp);
        self.set_pixel(x, y, c);
    }

    fn invert_region(&mut self, rect: &Rectangle) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let cur = self.get_pixel(x as u32, y as u32);
                let color = 255 - cur;
                self.set_pixel(x as u32, y as u32, color);
            }
        }
    }

    fn shift_region(&mut self, rect: &Rectangle, drift: u8) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let cur = self.get_pixel(x as u32, y as u32);
                let color = cur.saturating_sub(drift);
                self.set_pixel(x as u32, y as u32, color);
            }
        }
    }

    // Tell the driver that the screen needs to be redrawn.
    fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> Result<u32, Error> {
        let mut flags = 0;
        let mut monochrome = self.monochrome;

        let mut waveform_mode = match mode {
            UpdateMode::Gui => EINK_GL16_MODE,
            UpdateMode::Partial => EINK_GLR16_MODE,
            UpdateMode::Full => {
                monochrome = false;
                EINK_GC16_MODE
            },
            UpdateMode::Fast => EINK_A2_MODE,
            UpdateMode::FastMono => {
                flags |= EINK_MONOCHROME;
                EINK_A2_MODE
            },
        };

        if self.inverted {
            if waveform_mode == EINK_GL16_MODE || waveform_mode == EINK_GLR16_MODE {
                waveform_mode = EINK_GLK16_MODE;
            } else if waveform_mode == EINK_GC16_MODE {
                waveform_mode = EINK_GCK16_MODE;
            }
        }

        if mode != UpdateMode::Full && waveform_mode != EINK_AUTO_MODE {
            flags |= EINK_PARTIAL_MODE;
        }

        if waveform_mode == EINK_A2_MODE {
            flags |= EINK_MONOCHROME;
        }

        if mode == UpdateMode::Full {
            flags |= EINK_NO_MERGE;
        }

        if waveform_mode == EINK_GLR16_MODE || waveform_mode == EINK_GLD16_MODE {
            flags |= EINK_REGAL_MODE;
        }

        if monochrome && waveform_mode != EINK_A2_MODE {
            waveform_mode = EINK_DU_MODE;
            if !self.dithered {
                flags |= EINK_DITHERING_Y1;
            }
        }

        let area: AreaInfo = (*rect).into();

        let mut update_data = SunxiDispEinkUpdate2 {
            area: &area,
            layer_num: 1,
            update_mode: (waveform_mode | flags) as libc::c_ulong,
            lyr_cfg2: &self.layer,
            frame_id: &mut self.token as *mut libc::c_uint,
            rotate: &(90 * self.rotation() as u32),
            cfa_use: 0,
        };

        let result = unsafe {
            send_update(self.display.as_raw_fd(), &mut update_data)
        };

        match result {
            Err(e) => Err(Error::from(e).context("can't send framebuffer update")),
            _ => {
                Ok(self.token)
            }
        }
    }

    // Wait for a specific update to complete.
    fn wait(&self, token: u32) -> Result<i32, Error> {
        let marker_data = SunxiDispEinkWaitFrameSyncComplete {
            frame_id: token,
        };
        let result = unsafe {
            wait_for_update(self.display.as_raw_fd(), &marker_data)
        };
        result.context("can't wait for framebuffer update")
    }

    fn save(&self, path: &str) -> Result<(), Error> {
        let (width, height) = self.dims();
        let file = File::create(path).with_context(|| format!("can't create output file {}", path))?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_color(png::ColorType::Grayscale);
        let mut writer = encoder.write_header().with_context(|| format!("can't write PNG header for {}", path))?;
        writer.write_image_data(self.as_bytes()).with_context(|| format!("can't write PNG data to {}", path))?;
        Ok(())
    }

    #[inline]
    fn rotation(&self) -> i8 {
        self.var_info.rotate as i8
    }

    fn set_rotation(&mut self, n: i8) -> Result<(u32, u32), Error> {
        let delta = (self.rotation() - n).abs();

        if delta % 2 == 1 {
            mem::swap(&mut self.var_info.xres, &mut self.var_info.yres);
            mem::swap(&mut self.var_info.xres_virtual, &mut self.var_info.yres_virtual);
            mem::swap(&mut self.layer.info.screen_win.width, &mut self.layer.info.screen_win.height);
            unsafe {
                let rect = &mut (*self.layer.info.color_fb.fb).size[0];
                mem::swap(&mut rect.width,
                          &mut rect.height);
                let rect = &mut (*self.layer.info.color_fb.fb).crop;
                mem::swap(&mut rect.width,
                          &mut rect.height);
            }
            self.fix_info.line_length = self.var_info.xres_virtual;
            self.fix_info.smem_len = self.fix_info.line_length * self.var_info.yres_virtual;
            self.frame_size = (self.var_info.yres * self.fix_info.line_length) as usize;
        }

        self.var_info.rotate = n as u32;
        Ok((self.var_info.xres, self.var_info.yres))
    }

    fn set_inverted(&mut self, enable: bool) {
        if enable == self.inverted {
            return;
        }

        self.inverted = enable;
    }

    fn inverted(&self) -> bool {
        self.inverted
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

        if enable {
            self.transform = transform_dither_g16;
        } else {
            self.transform = transform_identity;
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

impl Drop for KoboFramebuffer2 {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.frame, self.alloc_size);
            libc::close(self.fd_data.fd);
            let mut data = IonHandleData { handle: self.fd_data.handle };
            let _ = ion_free(self.ion.as_raw_fd(), &mut data);
            ManuallyDrop::drop(&mut self.layer.info.color_fb.fb);
        }
    }
}
