extern crate libc;

use super::djvulibre_sys::*;

use std::ptr;
use std::rc::Rc;
use std::path::Path;
use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use super::{Document, Location, BoundedText, TocEntry};
use framebuffer::Pixmap;
use geom::Rectangle;

impl Into<DjvuRect> for Rectangle {
    fn into(self) -> DjvuRect {
        DjvuRect {
            x: self.min.y as libc::c_int,
            y: self.min.x as libc::c_int,
            w: self.width() as libc::c_uint,
            h: self.height() as libc::c_uint,
        }
    }
}

struct DjvuContext(*mut ExoContext);

pub struct DjvuOpener(Rc<DjvuContext>);

pub struct DjvuDocument {
    ctx: Rc<DjvuContext>,
    doc: *mut ExoDocument,
}

pub struct DjvuPage<'a> {
    page: *mut ExoPage,
    doc: &'a DjvuDocument,
}

impl DjvuContext {
    fn handle_message(&self) {
        unsafe {
            let msg = ddjvu_message_wait(self.0);
            if (*msg).tag == DDJVU_ERROR {
                let msg = (*msg).u.error;
                let message = CStr::from_ptr(msg.message).to_string_lossy();
                let filename = msg.filename;
                let lineno = msg.lineno;
                if filename.is_null() {
                    eprintln!("Error: {}.", message);
                } else {
                    let filename = CStr::from_ptr(filename).to_string_lossy();
                    eprintln!("Error: {}: '{}:{}'.", message, filename, lineno);
                }
            }
            ddjvu_message_pop(self.0);
        }
    }
}

impl DjvuOpener {
    pub fn new() -> Option<DjvuOpener> {
        unsafe {
            let name = CString::new("Plato").unwrap();
            let ctx = ddjvu_context_create(name.as_ptr());
            if ctx.is_null() {
                None
            } else {
                ddjvu_cache_set_size(ctx, CACHE_SIZE);
                Some(DjvuOpener(Rc::new(DjvuContext(ctx))))
            }
        }
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> Option<DjvuDocument> {
        unsafe {
            let c_path = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
            let doc = ddjvu_document_create_by_filename_utf8((self.0).0,
                                                             c_path.as_ptr(),
                                                             1);
            if doc.is_null() {
                return None;
            }
            let job = ddjvu_document_job(doc);
            while ddjvu_job_status(job) < DDJVU_JOB_OK {
                self.0.handle_message();
            }
            if ddjvu_job_status(job) >= DDJVU_JOB_FAILED {
                None
            } else {
                Some(DjvuDocument {
                    ctx: self.0.clone(),
                    doc,
                })
            }
        }
    }
}

unsafe impl Send for DjvuDocument {}
unsafe impl Sync for DjvuDocument {}

impl Document for DjvuDocument {
    fn dims(&self, index: usize) -> Option<(f32, f32)> {
        self.page(index).map(|page| {
            let dims = page.dims();
            (dims.0 as f32, dims.1 as f32)
        })
    }

    fn pages_count(&self) -> usize {
        unsafe { ddjvu_document_get_pagenum(self.doc) as usize }
    }

    fn pixmap(&mut self, loc: Location, scale: f32) -> Option<(Pixmap, usize)> {
        let index = self.resolve_location(loc)? as usize;
        self.page(index).and_then(|page| page.pixmap(scale)).map(|pixmap| (pixmap, index))
    }

    fn toc(&mut self) -> Option<Vec<TocEntry>> {
        unsafe {
            let mut exp = ddjvu_document_get_outline(self.doc);
            while exp == MINIEXP_DUMMY {
                self.ctx.handle_message();
                exp = ddjvu_document_get_outline(self.doc);
            }
            if exp == MINIEXP_NIL {
                None
            } else {
                let toc = Self::walk_toc(exp);
                ddjvu_miniexp_release(self.doc, exp);
                Some(toc)
            }
        }
    }

    fn words(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        unsafe {
            let index = self.resolve_location(loc)?;
            let page = self.page(index)?;
            let height = page.height() as i32;
            let grain = CString::new("word").unwrap();
            let mut exp = ddjvu_document_get_pagetext(self.doc, index as libc::c_int, grain.as_ptr());
            while exp == MINIEXP_DUMMY {
                self.ctx.handle_message();
                exp = ddjvu_document_get_pagetext(self.doc, index as libc::c_int, grain.as_ptr());
            }
            if exp == MINIEXP_NIL {
                None
            } else {
                let mut words = Vec::new();
                Self::walk_words(exp, height, &mut words);
                ddjvu_miniexp_release(self.doc, exp);
                Some((words, index))
            }
        }
    }

    fn links(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        unsafe {
            let index = self.resolve_location(loc)?;
            let mut exp = ddjvu_document_get_pageanno(self.doc, index as libc::c_int);
            while exp == MINIEXP_DUMMY {
                self.ctx.handle_message();
                exp = ddjvu_document_get_pageanno(self.doc, index as libc::c_int);
            }
            if exp == MINIEXP_NIL {
                None
            } else {
                let links = ddjvu_anno_get_hyperlinks(exp);
                if links.is_null() {
                    ddjvu_miniexp_release(self.doc, exp);
                    return None;
                }
                let height = self.page(index).map(|p| p.height()).unwrap() as i32;
                let c_rect = CString::new("rect").unwrap();
                let s_rect = miniexp_symbol(c_rect.as_ptr()) as *mut MiniExp;
                let mut link = links;
                let mut result = Vec::new();
                while !(*link).is_null() {
                    let uri = miniexp_nth(1, *link);
                    let area = miniexp_nth(3, *link);
                    if miniexp_stringp(uri) == 1 && miniexp_nth(0, area) == s_rect {
                        let text = CStr::from_ptr(miniexp_to_str(uri)).to_string_lossy().into_owned();
                        let rect = {
                            let x_min = miniexp_nth(1, area) as i32 >> 2;
                            let y_max = height - (miniexp_nth(2, area) as i32 >> 2);
                            let r_width = miniexp_nth(3, area) as i32 >> 2;
                            let r_height = miniexp_nth(4, area) as i32 >> 2;
                            rect![x_min, y_max - r_height, x_min + r_width, y_max]
                        };
                        result.push(BoundedText { text, rect });
                    }
                    link = link.offset(1);
                }
                libc::free(links as *mut libc::c_void);
                ddjvu_miniexp_release(self.doc, exp);
                Some((result, index))
            }
        }
    }

    fn metadata(&self, key: &str) -> Option<String> {
        unsafe {
            let mut exp = ddjvu_document_get_anno(self.doc, 1);
            while exp == MINIEXP_DUMMY {
                self.ctx.handle_message();
                exp = ddjvu_document_get_anno(self.doc, 1);
            }
            if exp == MINIEXP_NIL {
                None
            } else {
                let key = CString::new(key).unwrap();
                let key = miniexp_symbol(key.as_ptr());
                let val = ddjvu_anno_get_metadata(exp, key);
                if val.is_null() {
                    None
                } else {
                    ddjvu_miniexp_release(self.doc, exp);
                    Some(CStr::from_ptr(val).to_string_lossy().into_owned())
                }
            }
        }
    }

    fn title(&self) -> Option<String> {
        self.metadata("title")
    }

    fn author(&self) -> Option<String> {
        self.metadata("author")
    }

    fn is_reflowable(&self) -> bool {
        false
    }

    fn layout(&mut self, _width: u32, _height: u32, _font_size: f32, _dpi: u16) {
    }

    fn set_font_family(&mut self, _family_name: &str, _search_path: &str) {
    }

    fn set_margin_width(&mut self, _width: i32) {
    }

    fn set_line_height(&mut self, _line_height: f32) {
    }
}

impl DjvuDocument {
    pub fn page(&self, index: usize) -> Option<DjvuPage> {
        unsafe {
            let page = ddjvu_page_create_by_pageno(self.doc, index as libc::c_int);
            if page.is_null() {
                return None;
            }
            let job = ddjvu_page_job(page);
            while ddjvu_job_status(job) < DDJVU_JOB_OK {
                self.ctx.handle_message();
            }
            if ddjvu_job_status(job) >= DDJVU_JOB_FAILED {
                None
            } else {
                Some(DjvuPage { page, doc: self })
            }
        }
    }

    fn walk_words(exp: *mut MiniExp, height: i32, words: &mut Vec<BoundedText>) {
        unsafe {
            let len = miniexp_length(exp);
            let rect = {
                let x_min = miniexp_nth(1, exp) as i32 >> 2;
                let y_max = height - (miniexp_nth(2, exp) as i32 >> 2);
                let x_max = miniexp_nth(3, exp) as i32 >> 2;
                let y_min = height - (miniexp_nth(4, exp) as i32 >> 2);
                rect![x_min, y_min, x_max, y_max]
            };
            let grain = {
                let raw = miniexp_to_name(miniexp_nth(0, exp));
                CStr::from_ptr(raw).to_bytes()
            };
            if grain == b"word" && miniexp_stringp(miniexp_nth(5, exp)) == 1 {
                let raw = miniexp_to_str(miniexp_nth(5, exp));
                let c_str = CStr::from_ptr(raw);
                let text = c_str.to_string_lossy().into_owned();
                words.push(BoundedText { rect, text });
            } else {
                for i in 5..len {
                    Self::walk_words(miniexp_nth(i, exp), height, words);
                }
            }
        }
    }

    fn walk_toc(exp: *mut MiniExp) -> Vec<TocEntry> {
        unsafe {
            let mut vec = Vec::new();
            let len = miniexp_length(exp);
            for i in 0..len {
                let itm = miniexp_nth(i, exp);
                // Skip `itm` if it isn't a list.
                if (itm as libc::size_t) & 3 != 0 {
                    continue;
                }
                let raw = miniexp_to_str(miniexp_nth(0, itm));
                let title = CStr::from_ptr(raw).to_string_lossy().into_owned();
                let raw = miniexp_to_str(miniexp_nth(1, itm));
                let bytes = CStr::from_ptr(raw).to_bytes();
                // TODO: handle the case #page_name: we need to call ddjvu_document_get_fileinfo
                // for every file and try to find a matching page_name
                let digits = bytes.iter().map(|v| *v as u8 as char)
                                         .filter(|c| c.is_digit(10))
                                         .collect::<String>();
                let location = digits.parse::<usize>().unwrap_or(1).saturating_sub(1);
                let children = if miniexp_length(itm) > 2 {
                    Self::walk_toc(itm)
                } else {
                    Vec::new()
                };
                vec.push(TocEntry { title, location, children });
            }
            vec
        }
    }
}

impl<'a> DjvuPage<'a> {
    pub fn pixmap(&self, scale: f32) -> Option<Pixmap> {
        unsafe {
            let (width, height) = self.dims();
            let rect = DjvuRect {
                x: 0,
                y: 0,
                w: (scale * width as f32) as libc::c_uint,
                h: (scale * height as f32) as libc::c_uint,
            };

            let fmt = ddjvu_format_create(DDJVU_FORMAT_GREY8, 0, ptr::null());

            if fmt.is_null() {
                return None;
            }

            ddjvu_format_set_row_order(fmt, 1);
            ddjvu_format_set_y_direction(fmt, 1);

            let len = (rect.w * rect.h) as usize;
            let mut data = vec![0xff; len];

            ddjvu_page_render(self.page, DDJVU_RENDER_COLOR,
                              &rect, &rect, fmt,
                              rect.w as libc::c_ulong, data.as_mut_ptr());

            let job = ddjvu_page_job(self.page);

            while ddjvu_job_status(job) < DDJVU_JOB_OK {
                self.doc.ctx.handle_message();
            }

            ddjvu_format_release(fmt);

            if ddjvu_job_status(job) >= DDJVU_JOB_FAILED {
                return None;
            }

            Some(Pixmap { width: rect.w as u32,
                          height: rect.h as u32,
                          data })
        }
    }

    pub fn dims(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    pub fn width(&self) -> u32 {
        unsafe { ddjvu_page_get_width(self.page) as u32 }
    }

    pub fn height(&self) -> u32 {
        unsafe { ddjvu_page_get_height(self.page) as u32 }
    }

    pub fn dpi(&self) -> u16 {
        unsafe { ddjvu_page_get_resolution(self.page) as u16 }
    }
}

impl<'a> Drop for DjvuPage<'a> {
    fn drop(&mut self) {
        unsafe { ddjvu_job_release(ddjvu_page_job(self.page)); }
    }
}

impl Drop for DjvuDocument {
    fn drop(&mut self) {
        unsafe { ddjvu_job_release(ddjvu_document_job(self.doc)); }
    }
}

impl Drop for DjvuContext {
    fn drop(&mut self) {
        unsafe { ddjvu_context_release(self.0); }
    }
}
