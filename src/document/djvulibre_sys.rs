#![allow(unused)]

use std::mem;

pub const DDJVU_JOB_OK: JobStatus = 2;
pub const DDJVU_JOB_FAILED: JobStatus = 3;

pub const DDJVU_ERROR: MessageTag = 0;
pub const DDJVU_INFO: MessageTag = 1;
pub const DDJVU_NEWSTREAM: MessageTag = 2;
pub const DDJVU_DOCINFO: MessageTag = 3;
pub const DDJVU_PAGEINFO: MessageTag = 4;
pub const DDJVU_RELAYOUT: MessageTag = 5;
pub const DDJVU_REDISPLAY: MessageTag = 6;
pub const DDJVU_CHUNK: MessageTag = 7;
pub const DDJVU_THUMBNAIL: MessageTag = 8;
pub const DDJVU_PROGRESS: MessageTag = 9;

pub const DDJVU_FORMAT_BGR24: FormatStyle = 0;
pub const DDJVU_FORMAT_RGB24: FormatStyle = 1;
pub const DDJVU_FORMAT_RGBMASK16: FormatStyle = 2;
pub const DDJVU_FORMAT_RGBMASK32: FormatStyle = 3;
pub const DDJVU_FORMAT_GREY8: FormatStyle = 4;

pub const DDJVU_RENDER_COLOR: RenderMode = 0;

pub const MINIEXP_NIL: *mut MiniExp = 0 as *mut MiniExp;
pub const MINIEXP_DUMMY: *mut MiniExp = 2 as *mut MiniExp;

pub const CACHE_SIZE: libc::c_ulong = 32 * 1024 * 1024;

pub enum ExoContext {}
pub enum ExoDocument {}
pub enum ExoFormat {}
pub enum ExoJob {}
pub enum ExoPage {}
pub enum MiniExp {}

pub type JobStatus = libc::c_uint;
pub type MessageTag = libc::c_uint;
pub type RenderMode = libc::c_uint;
pub type FormatStyle = libc::c_uint;

#[link(name="djvulibre")]
extern {
    pub fn ddjvu_context_create(name: *const libc::c_char) -> *mut ExoContext;
    pub fn ddjvu_context_release(ctx: *mut ExoContext);
    pub fn ddjvu_cache_set_size(ctx: *mut ExoContext, size: libc::c_ulong);
    pub fn ddjvu_cache_clear(ctx: *mut ExoContext);
    pub fn ddjvu_message_wait(ctx: *mut ExoContext) -> *mut Message;
    pub fn ddjvu_message_pop(ctx: *mut ExoContext);
    pub fn ddjvu_document_job(doc: *mut ExoDocument) -> *mut ExoJob;
    pub fn ddjvu_page_job(page: *mut ExoPage) -> *mut ExoJob;
    pub fn ddjvu_job_status(job: *mut ExoJob) -> JobStatus;
    pub fn ddjvu_job_release(job: *mut ExoJob);
    pub fn ddjvu_document_create_by_filename_utf8(ctx: *mut ExoContext, path: *const libc::c_char, cache: libc::c_int) -> *mut ExoDocument;
    pub fn ddjvu_document_get_pagenum(doc: *mut ExoDocument) -> libc::c_int;
    pub fn ddjvu_page_create_by_pageno(doc: *mut ExoDocument, page_idx: libc::c_int) -> *mut ExoPage;
    pub fn ddjvu_page_create_by_pageid(doc: *mut ExoDocument, pageid: *const libc::c_char) -> *mut ExoPage;
    pub fn ddjvu_page_get_width(page: *mut ExoPage) -> libc::c_int;
    pub fn ddjvu_page_get_height(page: *mut ExoPage) -> libc::c_int;
    pub fn ddjvu_page_get_resolution(page: *mut ExoPage) -> libc::c_int;
    pub fn ddjvu_page_get_rotation(page: *mut ExoPage) -> libc::c_uint;
    pub fn ddjvu_page_render(page: *mut ExoPage, mode: RenderMode, p_rect: *const DjvuRect, r_rect: *const DjvuRect, fmt: *const ExoFormat, row_size: libc::c_ulong, buf: *mut u8) -> libc::c_int;
    pub fn ddjvu_format_create(style: FormatStyle, nargs: libc::c_int, args: *const libc::c_uint) -> *mut ExoFormat;
    pub fn ddjvu_format_release(fmt: *mut ExoFormat);
    pub fn ddjvu_format_set_row_order(fmt: *mut ExoFormat, top_to_bottom: libc::c_int);
    pub fn ddjvu_format_set_y_direction(fmt: *mut ExoFormat, top_to_bottom: libc::c_int);
    pub fn ddjvu_document_get_pagetext(doc: *mut ExoDocument, page_idx: libc::c_int, max_detail: *const libc::c_char) -> *mut MiniExp;
    pub fn ddjvu_document_get_outline(doc: *mut ExoDocument) -> *mut MiniExp;
    pub fn ddjvu_document_get_anno(doc: *mut ExoDocument, compat: libc::c_int) -> *mut MiniExp;
    pub fn ddjvu_document_get_pageanno(doc: *mut ExoDocument, page_idx: libc::c_int) -> *mut MiniExp;
    pub fn ddjvu_anno_get_hyperlinks(annot: *mut MiniExp) -> *mut *mut MiniExp;
    pub fn ddjvu_anno_get_metadata_keys(annot: *mut MiniExp) -> *mut *mut MiniExp;
    pub fn ddjvu_anno_get_metadata(annot: *mut MiniExp, key: *const MiniExp) -> *const libc::c_char;
    pub fn ddjvu_miniexp_release(document: *mut ExoDocument, exp: *mut MiniExp);
    pub fn miniexp_symbol(s: *const libc::c_char) -> *const MiniExp;
    pub fn miniexp_length(exp: *mut MiniExp) -> libc::c_int;
    pub fn miniexp_nth(n: libc::c_int, list: *mut MiniExp) -> *mut MiniExp;
    pub fn miniexp_stringp(exp: *mut MiniExp) -> libc::c_int;
    pub fn miniexp_to_str(exp: *mut MiniExp) -> *const libc::c_char;
    pub fn miniexp_to_name(sym: *mut MiniExp) -> *const libc::c_char;
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DjvuRect {
    pub x: libc::c_int,
    pub y: libc::c_int,
    pub w: libc::c_uint,
    pub h: libc::c_uint,
}

impl Default for DjvuRect {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

#[repr(C)]
pub struct Message {
    pub tag: MessageTag,
    context: *mut ExoContext,
    document: *mut ExoDocument,
    page: *mut ExoPage,
    job: *mut ExoJob,
    pub u: MessageBlob,
}

#[repr(C)]
pub union MessageBlob {
    pub error: MessageError,
    info: MessageInfo,
    new_stream: MessageNewStream,
    chunk: MessageChunk,
    thumbnail: MessageThumbnail,
    progress: MessageProgress,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MessageError {
    pub message: *const libc::c_char,
    function: *const libc::c_char,
    pub filename: *const libc::c_char,
    pub lineno: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MessageInfo {
    message: *const libc::c_char,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MessageNewStream {
    streamid: libc::c_int,
    name: *const libc::c_char,
    url: *const libc::c_char,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MessageChunk {
    chunkid: *const libc::c_char,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MessageThumbnail {
    pagenum: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MessageProgress {
    status: JobStatus,
    percent: libc::c_int,
}
