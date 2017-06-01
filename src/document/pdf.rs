extern crate libc;

use std::ptr;
use std::mem;
use std::slice;
use std::char;
use std::rc::Rc;
use std::path::Path;
use std::ffi::{CString, CStr};
use std::os::unix::ffi::OsStrExt;
use document::{TextLayer, LayerGrain, TocEntry};
use geom::Rectangle;

const CACHE_SIZE: libc::size_t = 32 * 1024 * 1024;
const FZ_MAX_COLORS: usize = 32;
const FZ_VERSION: &str = "1.11";

enum Context {}
enum Document {}
enum Page {}
enum Device {}
enum AllocContext {}
enum LocksContext {}
enum TextOptions {}
enum Cookie {}

#[link(name="mupdf", kind="static")]
#[link(name="mupdfwrapper", kind="static")]
extern {
    fn fz_new_context_imp(alloc_ctx: *const AllocContext, locks_ctx: *const LocksContext, cache_size: libc::size_t, version: *const libc::c_char) -> *mut Context;
    fn fz_drop_context(ctx: *mut Context);
    fn fz_register_document_handlers(ctx: *mut Context);
    fn mp_open_document(ctx: *mut Context, path: *const libc::c_char) -> *mut Document;
    fn fz_drop_document(ctx: *mut Context, doc: *mut Document);
    fn mp_count_pages(ctx: *mut Context, doc: *mut Document) -> libc::c_int;
    fn fz_needs_password(ctx: *mut Context, doc: *mut Document) -> libc::c_int;
    fn fz_is_document_reflowable(ctx: *mut Context, doc: *mut Document) -> libc::c_int;
    fn mp_load_outline(ctx: *mut Context, doc: *mut Document) -> *mut Outline;
    fn fz_drop_outline(ctx: *mut Context, outline: *mut Outline);
    fn fz_device_rgb(ctx: *mut Context) -> *mut FzColorspace;
    fn fz_device_gray(ctx: *mut Context) -> *mut FzColorspace;
    fn fz_scale(mat: *mut FzMatrix, sx: libc::c_float, sy: libc::c_float);
    fn fz_new_pixmap_from_page_number(ctx: *mut Context, doc: *mut Document, page_idx: libc::c_int, mat: *const FzMatrix, cs: *mut FzColorspace, alpha: libc::c_int) -> *mut FzPixmap;
    fn fz_drop_pixmap(ctx: *mut Context, pixmap: *mut FzPixmap);
    fn mp_load_page(ctx: *mut Context, doc: *mut Document, page_idx: libc::c_int) -> *mut Page;
    fn fz_drop_page(ctx: *mut Context, page: *mut Page);
    fn fz_bound_page(ctx: *mut Context, page: *mut Page, rect: *mut FzRect) -> *mut FzRect;
    fn fz_run_page(ctx: *mut Context, page: *mut Page, dev: *mut Device, mat: *const FzMatrix, cookie: *mut Cookie);
    fn fz_new_stext_page(ctx: *mut Context) -> *mut FzTextPage;
    fn fz_drop_stext_page(ctx: *mut Context, tp: *mut FzTextPage);
    fn fz_new_stext_sheet(ctx: *mut Context) -> *mut FzTextSheet;
    fn fz_drop_stext_sheet(ctx: *mut Context, tp: *mut FzTextSheet);
    fn fz_new_stext_device(ctx: *mut Context, ts: *mut FzTextSheet, tp: *mut FzTextPage, options: *const TextOptions) -> *mut Device;
    fn fz_new_bbox_device(ctx: *mut Context, rect: *mut FzRect) -> *mut Device;
    fn fz_stext_char_bbox(ctx: *mut Context, rect: *mut FzRect, span: *const FzTextSpan, idx: libc::c_int) -> *mut FzRect;
    fn fz_new_draw_device(ctx: *mut Context, mat: *const FzMatrix, pixmap: *mut FzPixmap) -> *mut Device;
    fn fz_new_draw_device_with_bbox(ctx: *mut Context, mat: *const FzMatrix, pixmap: *mut FzPixmap, clip: *const FzRect) -> *mut Device;
    fn fz_close_device(ctx: *mut Context, dev: *mut Device);
    fn fz_drop_device(ctx: *mut Context, dev: *mut Device);
    fn fz_new_pixmap(ctx: *mut Context, cs: *mut FzColorspace, width: libc::c_int, height: libc::c_int, alpha: libc::c_int) -> *mut FzPixmap;
    fn fz_new_pixmap_from_page(ctx: *mut Context, page: *mut Page, mat: *const FzMatrix, cs: *mut FzColorspace, alpha: libc::c_int) -> *mut FzPixmap;
    fn fz_clear_pixmap(ctx: *mut Context, pixmap: *mut FzPixmap);
    fn fz_union_rect(a: *mut FzRect, b: *const FzRect);
    fn fz_runetochar(buf: *mut u8, rune: libc::c_int) -> libc::c_int;
    static fz_identity: FzMatrix;
    static fz_resources_fonts_droid_DroidSansFallback_ttf_size: libc::c_int;
    static fz_resources_fonts_droid_DroidSansFallback_ttf: *const libc::c_char;
}

#[repr(C)]
#[derive(Debug, Clone)]
struct FzRect {
    x0: libc::c_float,
    y0: libc::c_float,
    x1: libc::c_float,
    y1: libc::c_float,
}

impl Into<FzRect> for Rectangle {
    fn into(self) -> FzRect {
        FzRect {
            x0: self.min.y as libc::c_float,
            y0: self.min.x as libc::c_float,
            x1: (self.max.x - 1) as libc::c_float,
            y1: (self.max.y - 1) as libc::c_float,
        }
    }
}

impl Into<Rectangle> for FzRect {
    fn into(self) -> Rectangle {
        rect![
            self.x0.floor() as i32,
            self.y0.floor() as i32,
            self.x1.ceil() as i32,
            self.y1.ceil() as i32,
        ]
    }
}

impl Default for FzRect {
    fn default() -> FzRect {
        unsafe { mem::zeroed() }
    }
}

#[repr(C)]
struct FzPoint {
    x: libc::c_float,
    y: libc::c_float,
}

#[repr(C)]
struct FzMatrix {
    a: libc::c_float,
    b: libc::c_float,
    c: libc::c_float,
    d: libc::c_float,
    e: libc::c_float,
    f: libc::c_float,
}

enum FzStoreDropFn {}

#[repr(C)]
struct FzStorable {
	refs: libc::c_int,
	drop: *mut FzStoreDropFn,
}

#[repr(C)]
struct FzPixmap {
	storable: FzStorable,
	x: libc::c_int,
	y: libc::c_int,
	w: libc::c_int,
	h: libc::c_int,
	n: libc::c_int,
	stride: libc::ptrdiff_t,
	alpha: libc::c_int,
	interpolate: libc::c_int,
	xres: libc::c_int,
	yres: libc::c_int,
	colorspace: *mut FzColorspace,
	samples: *mut u8,
	free_samples: libc::c_int,
}

impl Default for FzMatrix {
    fn default() -> FzMatrix {
        unsafe { mem::zeroed() }
    }
}

#[repr(C)]
struct FzTextSheet {
    maxid: libc::c_int,
    style: *mut FzTextStyle,
}

#[repr(C)]
struct FzTextPage {
    media_box: FzRect,
    len: libc::c_int,
    capacity: libc::c_int,
    blocks: *const PdfPageBlock,
    next: *const FzTextPage,
}

const FZ_PAGE_BLOCK_TEXT: libc::c_int = 0;
const FZ_PAGE_BLOCK_IMAGE: libc::c_int = 1;

#[repr(C)]
struct PdfPageBlock {
    kind: libc::c_int,
    data: *mut libc::c_void,
}

impl PdfPageBlock {
    fn text(&self) -> *const FzTextBlock {
        self.data as *const FzTextBlock
    }
    fn image(&self) -> *const FzImageBlock {
        self.data as *const FzImageBlock
    }
}

#[repr(C)]
struct FzTextBlock {
    bbox: FzRect,
    len: libc::c_int,
    cap: libc::c_int,
    lines: *mut FzTextLine,
}

#[repr(C)]
struct FzTextLine {
    first_span: *mut FzTextSpan,
    last_span: *mut FzTextSpan,
    distance: libc::c_float,
    bbox: FzRect,
    region: *mut libc::c_void,
}

#[repr(C)]
struct FzTextSpan {
    len: libc::c_int,
    cap: libc::c_int,
    text: *mut FzTextChar,
    min: FzPoint,
    max: FzPoint,
    wmode: libc::c_int,
    transform: FzMatrix,
    ascender_max: libc::c_float,
    descender_min: libc::c_float,
    bbox: FzRect,
    base_offset: libc::c_float,
    spacing: libc::c_float,
    column: libc::c_int,
    column_width: libc::c_float,
    align: libc::c_int,
    indent: libc::c_float,
    next: *mut FzTextSpan,
}

enum FzTextStyle {}

#[repr(C)]
struct FzTextChar {
    p: FzPoint,
    c: libc::c_int,
    style: *mut FzTextStyle,
}

#[repr(C)]
struct FzCharAndBox {
    c: libc::c_int,
    bbox: FzRect,
}

enum FzColorspace {}
enum FzImage {}

#[repr(C)]
struct FzImageBlock {
    bbox: FzRect,
    mat: FzMatrix,
    image: *mut FzImage,
    cspace: *mut FzColorspace,
    colors: [libc::c_float; FZ_MAX_COLORS],
}

#[repr(C)]
struct Outline {
    refs: libc::c_int,
    title: *mut libc::c_char,
    uri: *mut libc::c_char,
    page: libc::c_int,
    next: *mut Outline,
    down: *mut Outline,
    is_open: libc::c_int,
}

impl Default for Outline {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

struct PdfContext(*mut Context);

pub struct PdfOpener {
    ctx: Rc<PdfContext>,
}

pub struct PdfDocument {
    ctx: Rc<PdfContext>,
    doc: *mut Document,
}

pub struct PdfPage {
    ctx: Rc<PdfContext>,
    page: *mut Page,
}

impl PdfOpener {
    pub fn new() -> Option<PdfOpener> {
        unsafe {
            let version = CString::new(FZ_VERSION).unwrap();
            let ctx = fz_new_context_imp(ptr::null(), ptr::null(), CACHE_SIZE, version.as_ptr());
            if ctx.is_null() {
                None
            } else {
                fz_register_document_handlers(ctx);
                Some(PdfOpener {
                    ctx: Rc::new(PdfContext(ctx)),
                })
            }
        }
    }
    pub fn open<P: AsRef<Path>>(&self, path: P) -> Option<PdfDocument> {
        unsafe {
            let c_path = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
            let doc = mp_open_document(self.ctx.0, c_path.as_ptr());
            if doc.is_null() {
                None
            } else {
                Some(PdfDocument {
                    ctx: self.ctx.clone(),
                    doc: doc,
                })
            }
        }
    }
}

impl PdfDocument {
    pub fn pages_count(&self) -> Option<usize> {
        unsafe {
            let count = mp_count_pages(self.ctx.0, self.doc);
            if count < 0 {
                None
            } else {
                Some(count as usize)
            }
        }
    }
    pub fn page(&self, page_idx: usize) -> Option<PdfPage> {
        unsafe {
            let page = mp_load_page(self.ctx.0, self.doc, page_idx as libc::c_int);
            if page.is_null() {
                None
            } else {
                Some(PdfPage {
                    ctx: self.ctx.clone(),
                    page: page,
                })
            }
        }
    }
    pub fn toc(&self) -> Option<Vec<TocEntry>> {
        unsafe {
            let outline = mp_load_outline(self.ctx.0, self.doc);
            if outline.is_null() {
                None
            } else {
                let toc = Self::walk_toc(outline);
                fz_drop_outline(self.ctx.0, outline);
                Some(toc)
            }
        }
    }
    fn walk_toc(outline: *mut Outline) -> Vec<TocEntry> {
        unsafe {
            let mut vec = Vec::new();
            let mut cur = outline;
            while !cur.is_null() {
                let title = CStr::from_ptr((*cur).title).to_string_lossy().into_owned();
                // TODO: handle page == -1
                let page = (*cur).page as usize;
                let mut children = Vec::new();
                if !(*cur).down.is_null() {
                    children = Self::walk_toc((*cur).down);
                }
                vec.push(TocEntry {
                    title: title,
                    page: page,
                    children: children,
                });
                cur = (*cur).next;
            }
            vec
        }
    }
    pub fn is_reflowable(&self) -> bool {
        unsafe { fz_is_document_reflowable(self.ctx.0, self.doc) == 1 }
    }
    pub fn is_protected(&self) -> bool {
        unsafe { fz_needs_password(self.ctx.0, self.doc) == 1 }
    }
}

impl PdfPage {
    pub fn render(&self, scale: f32, rect: &mut Rectangle) -> Option<Vec<u8>> {
        unsafe {
            let mut mat = FzMatrix::default();
            fz_scale(&mut mat, scale as libc::c_float, scale as libc::c_float);
            let pixmap = fz_new_pixmap_from_page(self.ctx.0,
                                                 self.page,
                                                 &mat,
                                                 fz_device_gray(self.ctx.0),
                                                 0);
            if pixmap.is_null() {
                return None;
            }
            *rect = rect![0, 0, (*pixmap).w, (*pixmap).h];
            let len = (rect.width() * rect.height()) as usize;
            let mut vec = Vec::with_capacity(len);
            let slice = slice::from_raw_parts((*pixmap).samples, len);
            vec.extend_from_slice(slice);
            fz_drop_pixmap(self.ctx.0, pixmap);
            Some(vec)
        }
    }
    pub fn text(&self) -> Option<TextLayer> {
        unsafe {
            let tp = fz_new_stext_page(self.ctx.0);
            if tp.is_null() {
                return None;
            }
            let ts = fz_new_stext_sheet(self.ctx.0);
            if ts.is_null() {
                fz_drop_stext_page(self.ctx.0, tp);
                return None;
            }
            let dev = fz_new_stext_device(self.ctx.0, ts, tp, ptr::null()); 
            if dev.is_null() {
                fz_drop_stext_page(self.ctx.0, tp);
                fz_drop_stext_sheet(self.ctx.0, ts);
                return None;
            }
            fz_run_page(self.ctx.0, self.page, dev, &fz_identity, ptr::null_mut());
            fz_close_device(self.ctx.0, dev);
            fz_drop_device(self.ctx.0, dev);
            let blocks_count = (*tp).len as isize;
            let mut text_page = TextLayer {
                grain: LayerGrain::Page,
                rect: Rectangle::default(),
                children: vec![],
                text: None,
            };
            let mut page_rect = FzRect::default();
            #[inline]
            fn add_word(word: &mut String, word_rect: &mut FzRect, text_line: &mut TextLayer) {
                text_line.children.push(
                    TextLayer {
                        grain: LayerGrain::Word,
                        rect: word_rect.clone().into(),
                        children: vec![],
                        text: Some(word.clone()),
                    }
                );
                word.clear();
                *word_rect = FzRect::default();
            }
            for i in 0..blocks_count {
                let block = (*tp).blocks.offset(i);
                if (*block).kind == FZ_PAGE_BLOCK_TEXT {
                    let text_block = (*block).text();
                    fz_union_rect(&mut page_rect, &(*text_block).bbox);
                    let lines_count = (*text_block).len as isize;
                    for j in 0..lines_count {
                        let mut span = (*(*text_block).lines.offset(j)).first_span;
                        let mut text_line = TextLayer {
                            grain: LayerGrain::Line,
                            rect: (*(*text_block).lines.offset(j)).bbox.clone().into(),
                            children: vec![],
                            text: None,
                        };
                        let mut word = String::default();
                        let mut word_rect = FzRect::default();
                        while !span.is_null() {
                            let chars_count = (*span).len as isize;
                            for k in 0..chars_count {
                                let mut char_rect = FzRect::default();
                                fz_stext_char_bbox(self.ctx.0, &mut char_rect, span, k as libc::c_int);
                                if let Some(c) = char::from_u32((*(*span).text.offset(k)).c as u32) {
                                    if c.is_whitespace() {
                                        if !word.is_empty() {
                                            add_word(&mut word, &mut word_rect, &mut text_line);
                                        }
                                    } else {
                                        fz_union_rect(&mut word_rect, &char_rect);
                                        word.push(c);
                                    }
                                }
                            }
                            if !word.is_empty() {
                                add_word(&mut word, &mut word_rect, &mut text_line);
                            }
                            span = (*span).next;
                        }
                        if !word.is_empty() {
                            add_word(&mut word, &mut word_rect, &mut text_line);
                        }
                        if !text_line.children.is_empty() {
                            text_page.children.push(text_line);
                        }
                    }
                } else {
                    let image_block = (*block).image();
                    fz_union_rect(&mut page_rect, &(*image_block).bbox);
                }
            }
            text_page.rect = page_rect.into();
            fz_drop_stext_page(self.ctx.0, tp);
            fz_drop_stext_sheet(self.ctx.0, ts);
            Some(text_page)
        }
    }
    pub fn boundary_box(&self) -> Option<Rectangle> {
        unsafe {
            let mut rect = FzRect::default();
            let dev = fz_new_bbox_device(self.ctx.0, &mut rect);
            if dev.is_null() {
                None
            } else {
                fz_run_page(self.ctx.0, self.page, dev, &fz_identity, ptr::null_mut());
                fz_close_device(self.ctx.0, dev);
                fz_drop_device(self.ctx.0, dev);
                Some(rect.into())
            }
        }
    }
    pub fn dims(&self) -> (f32, f32) {
        unsafe {
            let mut bounds = FzRect::default();
            fz_bound_page(self.ctx.0, self.page, &mut bounds);
            ((bounds.x1 - bounds.x0) as f32, (bounds.y1 - bounds.y0) as f32)
        }
    }
    pub fn width(&self) -> f32 {
        let (width, _) = self.dims();
        width
    }
    pub fn height(&self) -> f32 {
        let (_, height) = self.dims();
        height
    }
}

impl Drop for PdfContext {
    fn drop(&mut self) {
        unsafe { fz_drop_context(self.0) };
    }
}

impl Drop for PdfDocument {
    fn drop(&mut self) {
        unsafe { fz_drop_document(self.ctx.0, self.doc) }
    }
}

impl Drop for PdfPage {
    fn drop(&mut self) {
        unsafe { fz_drop_page(self.ctx.0, self.page); }
    }
}
