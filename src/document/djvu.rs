extern crate libc;

use std::os::unix::ffi::OsStrExt;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::rc::Rc;
use std::mem;
use std::slice;
use std::ptr;
use geom::Rectangle;
use document::{TextLayer, LayerGrain, TocEntry};

const DDJVU_JOB_OK: libc::c_uint = 2;
const DDJVU_JOB_FAILED: libc::c_uint = 3;

const DDJVU_ERROR: libc::c_uint = 0;

const DDJVU_FORMAT_BGR24: libc::c_uint = 0;
const DDJVU_FORMAT_RGB24: libc::c_uint = 1;
const DDJVU_FORMAT_RGBMASK16: libc::c_uint = 2;
const DDJVU_FORMAT_RGBMASK32: libc::c_uint = 3;
const DDJVU_FORMAT_GREY8: libc::c_uint = 4;

const DDJVU_RENDER_COLOR: libc::c_uint = 0;
const MINIEXP_NIL: *mut MiniExp = 0 as *mut MiniExp;
const MINIEXP_DUMMY: *mut MiniExp = 2 as *mut MiniExp;

const CACHE_SIZE: libc::c_ulong = 32 * 1024 * 1024;

enum Context {}
enum Document {}
enum Message {}
enum Format {}
enum Job {}
enum Page {}
enum MiniExp {}

type Status = libc::c_uint;
type Mode = libc::c_uint;
type FormatStyle = libc::c_uint;

#[link(name="djvulibre")]
extern {
    fn ddjvu_context_create(name: *const libc::c_char) -> *mut Context;
    fn ddjvu_context_release(ctx: *mut Context);
    fn ddjvu_cache_set_size(ctx: *mut Context, size: libc::c_ulong);
    fn ddjvu_cache_clear(ctx: *mut Context);
    fn ddjvu_message_wait(ctx: *mut Context) -> *mut Message;
    fn ddjvu_message_pop(ctx: *mut Context);
    fn ddjvu_document_job(doc: *mut Document) -> *mut Job;
    fn ddjvu_page_job(page: *mut Page) -> *mut Job;
    fn ddjvu_job_status(job: *mut Job) -> Status;
    fn ddjvu_job_release(job: *mut Job);
    fn ddjvu_document_create_by_filename_utf8(ctx: *mut Context, path: *const libc::c_char, cache: libc::c_int) -> *mut Document;
    fn ddjvu_document_get_pagenum(doc: *mut Document) -> libc::c_int;
    fn ddjvu_page_create_by_pageno(doc: *mut Document, page_idx: libc::c_int) -> *mut Page;
    fn ddjvu_page_create_by_pageid(doc: *mut Document, pageid: *const libc::c_char) -> *mut Page;
    fn ddjvu_page_get_width(page: *mut Page) -> libc::c_int;
    fn ddjvu_page_get_height(page: *mut Page) -> libc::c_int;
    fn ddjvu_page_get_resolution(page: *mut Page) -> libc::c_int;
    fn ddjvu_page_get_rotation(page: *mut Page) -> libc::c_uint;
    fn ddjvu_page_render(page: *mut Page, mode: Mode, p_rect: *const DjvuRect, r_rect: *const DjvuRect, fmt: *const Format, row_size: libc::c_ulong, buf: *mut u8) -> libc::c_int;
    fn ddjvu_format_create(style: FormatStyle, nargs: libc::c_int, args: *const libc::c_uint) -> *mut Format;
    fn ddjvu_format_release(fmt: *mut Format);
    fn ddjvu_format_set_row_order(fmt: *mut Format, top_to_bottom: libc::c_int);
    fn ddjvu_format_set_y_direction(fmt: *mut Format, top_to_bottom: libc::c_int);
    fn ddjvu_document_get_pagetext(doc: *mut Document, page_idx: libc::c_int, max_detail: *const libc::c_char) -> *mut MiniExp;
    fn ddjvu_document_get_outline(doc: *mut Document) -> *mut MiniExp;
    fn ddjvu_document_get_anno(doc: *mut Document, compat: libc::c_int) -> *mut MiniExp;
    fn ddjvu_document_get_pageanno(doc: *mut Document, page_idx: libc::c_int) -> *mut MiniExp;
    fn ddjvu_anno_get_hyperlinks(annot: *mut MiniExp) -> *mut *mut MiniExp;
    fn ddjvu_anno_get_metadata_keys(annot: *mut MiniExp) -> *mut *mut MiniExp;
    fn ddjvu_anno_get_metadata(annot: *mut MiniExp, key: *mut MiniExp) -> *const libc::c_char;
    fn ddjvu_miniexp_release(document: *mut Document, exp: *mut MiniExp);
    fn miniexp_length(exp: *mut MiniExp) -> libc::c_int;
    fn miniexp_nth(n: libc::c_int, list: *mut MiniExp) -> *mut MiniExp;
    fn miniexp_stringp(exp: *mut MiniExp) -> libc::c_int;
    fn miniexp_to_str(exp: *mut MiniExp) -> *const libc::c_char;
    fn miniexp_to_name(sym: *mut MiniExp) -> *const libc::c_char;
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DjvuRect {
    pub x: libc::c_int,
    pub y: libc::c_int,
    pub w: libc::c_uint,
    pub h: libc::c_uint,
}

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

impl Default for DjvuRect {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

struct DjvuContext(*mut Context);

pub struct DjvuOpener {
    ctx: Rc<DjvuContext>,
}

pub struct DjvuDocument {
    ctx: Rc<DjvuContext>,
    doc: *mut Document,
}

pub struct DjvuPage(*mut Page);

impl DjvuContext {
    fn handle_message(&self) {
        unsafe {
            ddjvu_message_wait(self.0);
            ddjvu_message_pop(self.0);
        }
    }
}

impl LayerGrain {
    pub fn from_bytes(name: &[u8]) -> LayerGrain {
        match name {
            b"page" => LayerGrain::Page,
            b"column" => LayerGrain::Column,
            b"region" => LayerGrain::Region,
            b"para" => LayerGrain::Paragraph,
            b"line" => LayerGrain::Line,
            b"word" => LayerGrain::Word,
            b"char" => LayerGrain::Character,
            _ => LayerGrain::Character,
        }
    }
}

impl DjvuOpener {
    pub fn new() -> Option<DjvuOpener> {
        unsafe {
            let name = CString::new("plato").unwrap();
            let ctx = ddjvu_context_create(name.as_ptr());
            if ctx.is_null() {
                None
            } else {
                ddjvu_cache_set_size(ctx, CACHE_SIZE);
                Some(DjvuOpener {
                    ctx: Rc::new(DjvuContext(ctx)),
                })
            }
        }
    }
    pub fn open<P: AsRef<Path>>(&self, path: P) -> Option<DjvuDocument> {
        unsafe {
            let c_path = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
            let doc = ddjvu_document_create_by_filename_utf8(self.ctx.0,
                                                             c_path.as_ptr(),
                                                             1);
            if doc.is_null() {
                return None;
            }
            let job = ddjvu_document_job(doc);
            while ddjvu_job_status(job) < DDJVU_JOB_OK {
                self.ctx.handle_message();
            }
            if ddjvu_job_status(job) >= DDJVU_JOB_FAILED {
                None
            } else {
                Some(DjvuDocument {
                    ctx: self.ctx.clone(),
                    doc: doc,
                })
            }
        }
    }
}

impl DjvuPage {
    pub fn render<R: Into<DjvuRect>>(&self, p_rect: R, r_rect: R) -> Option<Vec<u8>> {
        unsafe {
            let r_rect = r_rect.into();
            let p_rect = p_rect.into();
            let fmt = ddjvu_format_create(DDJVU_FORMAT_GREY8, 0, ptr::null());

            ddjvu_format_set_row_order(fmt, 1);
            ddjvu_format_set_y_direction(fmt, 1);

            let len = r_rect.w * r_rect.h;
            let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
            ddjvu_page_render(self.0, DDJVU_RENDER_COLOR,
                              &p_rect, &r_rect, fmt,
                              r_rect.w as libc::c_ulong, buf.as_mut_ptr());
            buf.set_len(len as usize);
            ddjvu_format_release(fmt);
            Some(buf)
        }
    }
    pub fn dims(&self) -> (u32, u32) {
        (self.width(), self.height())
    }
    pub fn width(&self) -> u32 {
        unsafe { ddjvu_page_get_width(self.0) as u32 }
    }
    pub fn height(&self) -> u32 {
        unsafe { ddjvu_page_get_height(self.0) as u32 }
    }
    pub fn dpi(&self) -> u16 {
        unsafe { ddjvu_page_get_resolution(self.0) as u16 }
    }
}

impl DjvuDocument {
    pub fn pages_count(&self) -> usize {
        unsafe { ddjvu_document_get_pagenum(self.doc) as usize }
    }
    pub fn text(&self, page_idx: usize) -> Option<TextLayer> {
        unsafe {
            let page = self.page(page_idx);
            if page.is_none() {
                return None;
            }
            let height = page.unwrap().height() as i32;
            let grain = CString::new("word").unwrap();
            let mut exp = ddjvu_document_get_pagetext(self.doc, page_idx as libc::c_int, grain.as_ptr());
            while exp == MINIEXP_DUMMY {
                self.ctx.handle_message();
                exp = ddjvu_document_get_pagetext(self.doc, page_idx as libc::c_int, grain.as_ptr());
            }
            if exp == MINIEXP_NIL {
                None
            } else {
                let text_layer = Self::walk_text(exp, height);
                ddjvu_miniexp_release(self.doc, exp);
                Some(text_layer)
            }
        }
    }
    fn walk_text(exp: *mut MiniExp, height: i32) -> TextLayer {
        unsafe {
            let len = miniexp_length(exp);
            let mut text: Option<String> = None;
            let p_rect = {
                let min_x = miniexp_nth(1, exp) as i32 >> 2;
                let max_y = height - (miniexp_nth(2, exp) as i32 >> 2);
                let max_x = (miniexp_nth(3, exp) as i32 >> 2) + 1;
                let min_y = height - (miniexp_nth(4, exp) as i32 >> 2) - 1;
                rect![min_x, min_y, max_x, max_y]
            };
            let grain = {
                let raw = miniexp_to_name(miniexp_nth(0, exp));
                let c_str = CStr::from_ptr(raw);
                LayerGrain::from_bytes(c_str.to_bytes())
            };
            let mut children = Vec::new();
            if miniexp_stringp(miniexp_nth(5, exp)) == 1 {
                let raw = miniexp_to_str(miniexp_nth(5, exp));
                let c_str = CStr::from_ptr(raw);
                text = Some(c_str.to_string_lossy().into_owned());
            } else {
                for i in 5..len {
                    let child = miniexp_nth(i, exp);
                    children.push(Self::walk_text(child, height));
                }
            }
            TextLayer {
                grain: grain,
                rect: p_rect,
                text: text,
                children: children,
            }
        }
    }
    pub fn toc(&self) -> Option<Vec<TocEntry>> {
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
                let digits = bytes.iter().map(|v| *v as u8 as char)
                                         .filter(|c| c.is_digit(10))
                                         .collect::<String>();
                let page = digits.parse::<usize>().unwrap_or(1).saturating_sub(1);
                let mut children = Vec::new();
                if miniexp_length(itm) > 2 {
                    children = Self::walk_toc(itm);
                }
                vec.push(TocEntry {
                    title: title,
                    page: page,
                    children: children,
                });
            }
            vec
        }
    }
    pub fn page(&self, page_idx: usize) -> Option<DjvuPage> {
        unsafe {
            let page = ddjvu_page_create_by_pageno(self.doc, page_idx as libc::c_int);
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
                Some(DjvuPage(page))
            }
        }
    }
}

impl Drop for DjvuPage {
    fn drop(&mut self) {
        unsafe { ddjvu_job_release(ddjvu_page_job(self.0)); }
    }
}

impl Drop for DjvuDocument {
    fn drop(&mut self) {
        unsafe { ddjvu_job_release(ddjvu_document_job(self.doc)); }
    }
}

impl Drop for DjvuContext {
    fn drop(&mut self) {
        unsafe { ddjvu_context_release(self.0) };
    }
}
