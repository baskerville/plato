#![allow(unused)]

use std::mem;

pub const FZ_MAX_COLORS: usize = 32;
pub const FZ_VERSION: &str = "1.27.0";

pub const FZ_META_INFO_AUTHOR: &str = "info:Author";
pub const FZ_META_INFO_TITLE: &str = "info:Title";

pub const FZ_TEXT_PRESERVE_LIGATURES: libc::c_int = 1;
pub const FZ_TEXT_PRESERVE_WHITESPACE: libc::c_int = 2;
pub const FZ_TEXT_PRESERVE_IMAGES: libc::c_int = 4;
pub const FZ_TEXT_INHIBIT_SPACES: libc::c_int = 8;
pub const FZ_TEXT_DEHYPHENATE: libc::c_int = 16;
pub const FZ_TEXT_PRESERVE_SPANS: libc::c_int = 32;
pub const FZ_TEXT_CLIP: libc::c_int = 64;
pub const FZ_TEXT_CLIP_RECT: libc::c_int = 1<<17;
pub const FZ_TEXT_COLLECT_STRUCTURE: libc::c_int = 256;
pub const FZ_TEXT_COLLECT_VECTORS: libc::c_int = 1024;
pub const FZ_TEXT_ACCURATE_BBOXES: libc::c_int = 512;
pub const FZ_TEXT_ACCURATE_ASCENDERS: libc::c_int = 1<<18;
pub const FZ_TEXT_ACCURATE_SIDE_BEARINGS: libc::c_int = 1<<19;
pub const FZ_TEXT_IGNORE_ACTUALTEXT: libc::c_int = 2048;
pub const FZ_TEXT_SEGMENT: libc::c_int = 4096;
pub const FZ_TEXT_PARAGRAPH_BREAK: libc::c_int = 8192;
pub const FZ_TEXT_TABLE_HUNT: libc::c_int = 16384;
pub const FZ_TEXT_USE_CID_FOR_UNKNOWN_UNICODE: libc::c_int = 128;
pub const FZ_TEXT_USE_GID_FOR_UNKNOWN_UNICODE: libc::c_int = 65536;

pub const FZ_PAGE_BLOCK_TEXT: libc::c_int = 0;
pub const FZ_PAGE_BLOCK_IMAGE: libc::c_int = 1;
pub const FZ_PAGE_BLOCK_STRUCT: libc::c_int = 2;
pub const FZ_PAGE_BLOCK_VECTOR: libc::c_int = 3;
pub const FZ_PAGE_BLOCK_GRID: libc::c_int = 4;

pub const CACHE_SIZE: libc::size_t = 32 * 1024 * 1024;

pub enum FzContext {}
pub enum FzDocument {}
pub enum FzStream {}
pub enum FzPool {}
pub enum FzPage {}
pub enum FzDevice {}
pub enum FzFont {}
pub enum FzColorspace {}
pub enum FzAllocContext {}
pub enum FzLocksContext {}
pub enum FzCookie {}
pub enum FzStoreDropFn {}
pub enum FzStoreDroppableFn {}
pub enum FzLinkSetRectFn {}
pub enum FzLinkSetUriFn {}
pub enum FzLinkDropLinkFn {}
pub enum FzSeparations {}
pub enum FzTextStruct {}
pub enum FzGridPositions {}
pub enum FzGridInfo {}
pub enum FzPoolArray {}
pub enum FzImage {}

#[link(name="mupdf")]
#[link(name="mupdf_wrapper", kind="static")]

extern {
    pub fn fz_new_context_imp(alloc_ctx: *const FzAllocContext, locks_ctx: *const FzLocksContext, cache_size: libc::size_t, version: *const libc::c_char) -> *mut FzContext;
    pub fn fz_drop_context(ctx: *mut FzContext);
    pub fn fz_register_document_handlers(ctx: *mut FzContext);
    pub fn fz_set_user_css(ctx: *mut FzContext, user_css: *const libc::c_char);
    pub fn fz_set_use_document_css(ctx: *mut FzContext, should_use: libc::c_int);
    pub fn mp_open_document(ctx: *mut FzContext, path: *const libc::c_char) -> *mut FzDocument;
    pub fn mp_open_document_with_stream(ctx: *mut FzContext, kind: *const libc::c_char, stream: *mut FzStream) -> *mut FzDocument;
    pub fn fz_drop_document(ctx: *mut FzContext, doc: *mut FzDocument);
    pub fn fz_open_memory(ctx: *mut FzContext, data: *const libc::c_uchar, len: libc::size_t) -> *mut FzStream;
    pub fn fz_drop_stream(ctx: *mut FzContext, stream: *mut FzStream);
    pub fn mp_count_pages(ctx: *mut FzContext, doc: *mut FzDocument) -> libc::c_int;
    pub fn mp_page_number_from_location(ctx: *mut FzContext, doc: *mut FzDocument, loc: FzLocation) -> libc::c_int;
    pub fn fz_lookup_metadata(ctx: *mut FzContext, doc: *mut FzDocument, key: *const libc::c_char, buf: *mut libc::c_char, size: libc::c_int) -> libc::c_int;
    pub fn fz_needs_password(ctx: *mut FzContext, doc: *mut FzDocument) -> libc::c_int;
    pub fn fz_is_document_reflowable(ctx: *mut FzContext, doc: *mut FzDocument) -> libc::c_int;
    pub fn fz_layout_document(ctx: *mut FzContext, doc: *mut FzDocument, w: libc::c_float, h: libc::c_float, em: libc::c_float);
    pub fn mp_load_outline(ctx: *mut FzContext, doc: *mut FzDocument) -> *mut FzOutline;
    pub fn fz_drop_outline(ctx: *mut FzContext, outline: *mut FzOutline);
    pub fn fz_device_rgb(ctx: *mut FzContext) -> *mut FzColorspace;
    pub fn fz_device_gray(ctx: *mut FzContext) -> *mut FzColorspace;
    pub fn fz_scale(sx: libc::c_float, sy: libc::c_float) -> FzMatrix;
    pub fn mp_new_pixmap_from_page(ctx: *mut FzContext, page: *mut FzPage, mat: FzMatrix, cs: *mut FzColorspace, alpha: libc::c_int) -> *mut FzPixmap;
    pub fn fz_set_pixmap_resolution(ctx: *mut FzContext, pix: *mut FzPixmap, xres: libc::c_int, yres: libc::c_int);
    pub fn fz_drop_pixmap(ctx: *mut FzContext, pixmap: *mut FzPixmap);
    pub fn mp_load_page(ctx: *mut FzContext, doc: *mut FzDocument, page_idx: libc::c_int) -> *mut FzPage;
    pub fn fz_drop_page(ctx: *mut FzContext, page: *mut FzPage);
    pub fn fz_bound_page(ctx: *mut FzContext, page: *mut FzPage) -> FzRect;
    pub fn fz_run_page(ctx: *mut FzContext, page: *mut FzPage, dev: *mut FzDevice, mat: FzMatrix, cookie: *mut FzCookie);
    pub fn mp_load_links(ctx: *mut FzContext, page: *mut FzPage) -> *mut FzLink;
    pub fn fz_drop_link(ctx: *mut FzContext, link: *mut FzLink);
    pub fn fz_resolve_link_dest(ctx: *mut FzContext, doc: *mut FzDocument, uri: *const libc::c_char) -> FzLinkDest;
    pub fn mp_new_stext_page_from_page(ctx: *mut FzContext, page: *mut FzPage, options: *const FzTextOptions) -> *mut FzTextPage;
    pub fn fz_drop_stext_page(ctx: *mut FzContext, tp: *mut FzTextPage);
    pub fn fz_new_bbox_device(ctx: *mut FzContext, rect: *mut FzRect) -> *mut FzDevice;
    pub fn fz_close_device(ctx: *mut FzContext, dev: *mut FzDevice);
    pub fn fz_drop_device(ctx: *mut FzContext, dev: *mut FzDevice);
    pub fn fz_union_rect(a: FzRect, b: FzRect) -> FzRect;
    pub fn fz_rect_from_quad(q: FzQuad) -> FzRect;
    pub fn fz_runetochar(buf: *mut u8, rune: libc::c_int) -> libc::c_int;
    pub static fz_identity: FzMatrix;
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FzRect {
    pub x0: libc::c_float,
    pub y0: libc::c_float,
    pub x1: libc::c_float,
    pub y1: libc::c_float,
}

impl Default for FzRect {
    fn default() -> FzRect {
        // Returns an empty rectangle.
        FzRect {
            x0: 1.0,
            y0: 1.0,
            x1: -1.0,
            y1: -1.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FzPoint {
    x: libc::c_float,
    y: libc::c_float,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FzQuad {
    ul: FzPoint,
    ur: FzPoint,
    ll: FzPoint,
    lr: FzPoint,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FzMatrix {
    a: libc::c_float,
    b: libc::c_float,
    c: libc::c_float,
    d: libc::c_float,
    e: libc::c_float,
    f: libc::c_float,
}

#[repr(C)]
pub struct FzStorable {
    refs: libc::c_int,
    drop: *mut FzStoreDropFn,
    droppable: *mut FzStoreDroppableFn,
}

#[repr(C)]
pub struct FzTextOptions {
    pub flags: libc::c_int,
    pub scale: libc::c_float,
    pub clip: FzRect,
}

#[repr(C)]
pub struct FzLinkDest {
    pub loc: FzLocation,
    kind: libc::c_int,
    x: libc::c_float,
    y: libc::c_float,
    w: libc::c_float,
    h: libc::c_float,
    zoom: libc::c_float,
}

#[repr(C)]
pub struct FzPixmap {
    storable: FzStorable,
    x: libc::c_int,
    y: libc::c_int,
    pub w: libc::c_int,
    pub h: libc::c_int,
    n: libc::c_uchar,
    s: libc::c_uchar,
    alpha: libc::c_uchar,
    flags: libc::c_uchar,
    stride: libc::ptrdiff_t,
    seps: *mut FzSeparations,
    xres: libc::c_int,
    yres: libc::c_int,
    colorspace: *mut FzColorspace,
    pub samples: *mut libc::c_uchar,
    underlying: *mut FzPixmap,
}

impl Default for FzMatrix {
    fn default() -> FzMatrix {
        unsafe { mem::zeroed() }
    }
}

#[repr(C)]
pub struct FzLink {
    refs: libc::c_int,
    pub next: *mut FzLink,
    pub rect: FzRect,
    pub uri: *mut libc::c_char,
    set_rect_fn: *mut FzLinkSetRectFn,
    set_uri_fn: *mut FzLinkSetUriFn,
    drop: *mut FzLinkDropLinkFn,
}

#[repr(C)]
pub struct FzTextPage {
    refs: libc::c_int,
    pool: *mut FzPool,
    mediabox: FzRect,
    pub first_block: *mut FzTextBlock,
    last_block: *mut FzTextBlock,
    last_struct: *mut FzTextStruct,
    id_list: *mut FzPoolArray,
}

#[repr(C)]
pub struct FzTextBlock {
    pub kind: libc::c_int,
    id: libc::c_int,
    pub bbox: FzRect,
    pub u: FzTextBlockTextImage,
    prev: *mut FzTextBlock,
    pub next: *mut FzTextBlock,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FzTextBlockText {
    pub first_line: *mut FzTextLine,
    last_line: *mut FzTextLine,
    flags: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FzTextBlockImage {
    transform: FzMatrix,
    image: *mut FzImage,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FzTextBlockStruct{
    down: *mut FzTextStruct,
    index: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FzTextBlockVector {
    flags: u32,
    argb: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FzTextBlockGrid {
    xs: *mut FzGridPositions,
    ys: *mut FzGridPositions,
    info: *mut FzGridInfo,
}

#[repr(C)]
pub union FzTextBlockTextImage {
    pub text: FzTextBlockText,
    pub image: FzTextBlockImage,
    pub stru: FzTextBlockStruct,
    pub vector: FzTextBlockVector,
    pub grid: FzTextBlockGrid,
}

#[repr(C)]
pub struct FzTextLine {
    wmode: u8,
    flags: u8,
    dir: FzPoint,
    pub bbox: FzRect,
    pub first_char: *mut FzTextChar,
    last_char: *mut FzTextChar,
    prev: *mut FzTextLine,
    pub next: *mut FzTextLine,
}

#[repr(C)]
pub struct FzTextChar {
    pub c: libc::c_int,
    bidi: u16,
    flags: u16,
    argb: u32,
    origin: FzPoint,
    pub quad: FzQuad,
    size: libc::c_float,
    font: *mut FzFont,
    pub next: *mut FzTextChar,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FzLocation {
	pub chapter: libc::c_int,
	pub page: libc::c_int,
}

#[repr(C)]
pub struct FzOutline {
    refs: libc::c_int,
    pub title: *mut libc::c_char,
    pub uri: *mut libc::c_char,
    pub page: FzLocation,
    x: libc::c_float,
    y: libc::c_float,
    pub next: *mut FzOutline,
    pub down: *mut FzOutline,
    is_open: libc::c_uint,
    flags: libc::c_uint,
    r: libc::c_uint,
    g: libc::c_uint,
    b: libc::c_uint,
}

impl Default for FzOutline {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}
