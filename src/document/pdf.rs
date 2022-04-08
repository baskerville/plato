use super::mupdf_sys::*;

use std::ptr;
use std::slice;
use std::char;
use std::rc::Rc;
use std::path::Path;
use std::io::Read;
use std::fs::File;
use std::ffi::{CString, CStr};
use std::os::unix::ffi::OsStrExt;
use anyhow::Error;
use super::{Document, Location, TextLocation, BoundedText, TocEntry};
use super::{chapter, chapter_relative};
use crate::metadata::TextAlign;
use crate::geom::{Boundary, CycleDir};
use crate::unit::pt_to_px;
use crate::framebuffer::Pixmap;

impl Into<Boundary> for FzRect {
    fn into(self) -> Boundary {
        Boundary {
            min: vec2!(self.x0, self.y0),
            max: vec2!(self.x1, self.y1),
        }
    }
}

struct PdfContext(*mut FzContext);

pub struct PdfOpener(Rc<PdfContext>);

pub struct PdfDocument {
    ctx: Rc<PdfContext>,
    doc: *mut FzDocument,
}

pub struct PdfPage<'a> {
    ctx: Rc<PdfContext>,
    page: *mut FzPage,
    index: usize,
    _doc: &'a PdfDocument,
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
                Some(PdfOpener(Rc::new(PdfContext(ctx))))
            }
        }
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> Option<PdfDocument> {
        unsafe {
            let c_path = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
            let doc = mp_open_document((self.0).0, c_path.as_ptr());
            if doc.is_null() {
                None
            } else {
                Some(PdfDocument {
                    ctx: self.0.clone(),
                    doc,
                })
            }
        }
    }

    // *magic* is a filename or a MIME type.
    pub fn open_memory(&self, magic: &str, buf: &[u8]) -> Option<PdfDocument> {
        unsafe {
            let stream = fz_open_memory((self.0).0,
                                        buf.as_ptr() as *const libc::c_uchar,
                                        buf.len() as libc::size_t);
            let c_magic = CString::new(magic).unwrap();
            let doc = mp_open_document_with_stream((self.0).0, c_magic.as_ptr(), stream);
            fz_drop_stream((self.0).0, stream);
            if doc.is_null() {
                None
            } else {
                Some(PdfDocument {
                    ctx: self.0.clone(),
                    doc,
                })
            }
        }
    }

    pub fn set_user_css<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let mut file = File::open(path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        let buf = CString::new(buf)?;
        unsafe {
            // The CSS will only be parsed when an HTML or EPUB document is opened
            fz_set_user_css((self.0).0, buf.as_ptr());
        }
        Ok(())
    }
}

unsafe impl Send for PdfDocument {}
unsafe impl Sync for PdfDocument {}

impl PdfDocument {
    pub fn page(&self, index: usize) -> Option<PdfPage> {
        unsafe {
            let page = mp_load_page(self.ctx.0, self.doc, index as libc::c_int);
            if page.is_null() {
                None
            } else {
                Some(PdfPage {
                    ctx: self.ctx.clone(),
                    page,
                    index,
                    _doc: self,
                })
            }
        }
    }

    fn walk_toc(outline: *mut FzOutline, index: &mut usize) -> Vec<TocEntry> {
        unsafe {
            let mut vec = Vec::new();
            let mut cur = outline;
            while !cur.is_null() {
                let location = if (*cur).page > -1 {
                    Location::Exact((*cur).page as usize)
                } else if !(*cur).uri.is_null() {
                    let uri = CStr::from_ptr((*cur).uri).to_string_lossy().into_owned();
                    Location::Uri(uri)
                } else {
                    Location::Exact(0)
                };
                let title = if !(*cur).title.is_null() {
                    CStr::from_ptr((*cur).title).to_string_lossy().into_owned()
                } else {
                    "Untitled".to_string()
                };
                let current_index = *index;
                *index += 1;
                let children = if !(*cur).down.is_null() {
                    Self::walk_toc((*cur).down, index)
                } else {
                    Vec::new()
                };
                vec.push(TocEntry { title, location, index: current_index, children });
                cur = (*cur).next;
            }
            vec
        }
    }

    pub fn is_protected(&self) -> bool {
        unsafe { fz_needs_password(self.ctx.0, self.doc) == 1 }
    }
}

impl Document for PdfDocument {
    fn dims(&self, index: usize) -> Option<(f32, f32)> {
        self.page(index).map(|page| page.dims())
    }

    fn pages_count(&self) -> usize {
        unsafe { mp_count_pages(self.ctx.0, self.doc) as usize }
    }

    fn pixmap(&mut self, loc: Location, scale: f32) -> Option<(Pixmap, usize)> {
        let index = self.resolve_location(loc)?;
        self.page(index).and_then(|page| page.pixmap(scale)).map(|pixmap| (pixmap, index))
    }

    fn toc(&mut self) -> Option<Vec<TocEntry>> {
        unsafe {
            let outline = mp_load_outline(self.ctx.0, self.doc);
            if outline.is_null() {
                None
            } else {
                let mut index = 0;
                let toc = Self::walk_toc(outline, &mut index);
                fz_drop_outline(self.ctx.0, outline);
                Some(toc)
            }
        }
    }

    fn chapter<'a>(&mut self, offset: usize, toc: &'a [TocEntry]) -> Option<(&'a TocEntry, f32)> {
        chapter(offset, self.pages_count(), toc)
    }

    fn chapter_relative<'a>(&mut self, offset: usize, dir: CycleDir, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
        chapter_relative(offset, dir, toc)
    }

    fn metadata(&self, key: &str) -> Option<String> {
        unsafe {
            let key = CString::new(key).unwrap();
            let mut buf: [libc::c_char; 256] = [0; 256];
            let len = fz_lookup_metadata(self.ctx.0, self.doc, key.as_ptr(), buf.as_mut_ptr(), buf.len() as libc::c_int);
            if len == -1 {
                None
            } else {
                Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
            }
        }
    }

    fn words(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        let index = self.resolve_location(loc)?;
        self.page(index).and_then(|page| page.words()).map(|words| (words, index))
    }

    fn lines(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        let index = self.resolve_location(loc)?;
        self.page(index).and_then(|page| page.lines()).map(|lines| (lines, index))
    }

    fn images(&mut self, loc: Location) -> Option<(Vec<Boundary>, usize)> {
        let index = self.resolve_location(loc)?;
        self.page(index).and_then(|page| page.images()).map(|images| (images, index))
    }

    fn links(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        let index = self.resolve_location(loc)?;
        self.page(index).and_then(|page| page.links()).map(|links| (links, index))
    }

    fn title(&self) -> Option<String> {
        self.metadata(FZ_META_INFO_TITLE)
    }

    fn author(&self) -> Option<String> {
        self.metadata(FZ_META_INFO_AUTHOR)
    }

    fn is_reflowable(&self) -> bool {
        unsafe { fz_is_document_reflowable(self.ctx.0, self.doc) == 1 }
    }

    fn layout(&mut self, width: u32, height: u32, font_size: f32, dpi: u16) {
        let em = pt_to_px(font_size, dpi);
        unsafe {
            fz_layout_document(self.ctx.0, self.doc,
                               width as libc::c_float,
                               height as libc::c_float,
                               em as libc::c_float);
        }
    }

    fn set_text_align(&mut self, _text_align: TextAlign) {
    }

    fn set_font_family(&mut self, _family_name: &str, _search_path: &str) {
    }

    fn set_margin_width(&mut self, _width: i32) {
    }

    fn set_line_height(&mut self, _line_height: f32) {
    }

    fn set_hyphen_penalty(&mut self, _hyphen_penalty: i32) {
    }

    fn set_stretch_tolerance(&mut self, _stretch_tolerance: f32) {
    }

    fn set_ignore_document_css(&mut self, ignore: bool) {
        unsafe {
            fz_set_use_document_css(self.ctx.0, !ignore as libc::c_int);
        }
    }
}

impl<'a> PdfPage<'a> {
    pub fn images(&self) -> Option<Vec<Boundary>> {
        unsafe {
            let mut images: Vec<Boundary> = Vec::new();
            let opts = FzTextOptions { flags: FZ_TEXT_PRESERVE_IMAGES };
            let tp = mp_new_stext_page_from_page(self.ctx.0, self.page, &opts);
            if tp.is_null() {
                return None;
            }

            let mut block = (*tp).first_block;

            while !block.is_null() {
                if (*block).kind == FZ_PAGE_BLOCK_IMAGE {
                    let bnd: Boundary = (*block).bbox.into();
                    images.retain(|img| !img.overlaps(&bnd));
                    images.push(bnd);
                }

                block = (*block).next;
            }

            fz_drop_stext_page(self.ctx.0, tp);
            Some(images)
        }
    }

    pub fn lines(&self) -> Option<Vec<BoundedText>> {
        unsafe {
            let mut lines = Vec::new();
            let tp = mp_new_stext_page_from_page(self.ctx.0, self.page, ptr::null());
            if tp.is_null() {
                return None;
            }
            let mut offset = 0;
            let mut block = (*tp).first_block;

            while !block.is_null() {
                if (*block).kind == FZ_PAGE_BLOCK_TEXT {
                    let text_block = (*block).u.text;
                    let mut line = text_block.first_line;

                    while !line.is_null() {
                        let rect = (*line).bbox.into();
                        lines.push(BoundedText {
                            rect,
                            text: String::default(),
                            location: TextLocation::Static(self.index, offset),
                        });
                        offset += 1;
                        line = (*line).next;
                    }
                }

                block = (*block).next;
            }

            fz_drop_stext_page(self.ctx.0, tp);
            Some(lines)
        }
    }

    pub fn words(&self) -> Option<Vec<BoundedText>> {
        unsafe {
            let mut words = Vec::new();
            let tp = mp_new_stext_page_from_page(self.ctx.0, self.page, ptr::null());
            if tp.is_null() {
                return None;
            }
            let mut block = (*tp).first_block;
            let mut offset = 0;

            while !block.is_null() {
                if (*block).kind == FZ_PAGE_BLOCK_TEXT {
                    let text_block = (*block).u.text;
                    let mut line = text_block.first_line;

                    while !line.is_null() {
                        let mut chr = (*line).first_char;
                        let mut text = String::default();
                        let mut rect = FzRect::default();

                        while !chr.is_null() {
                            while !chr.is_null() {
                                if let Some(c) = char::from_u32((*chr).c as u32) {
                                    if c.is_whitespace() {
                                        chr = (*chr).next;
                                        break;
                                    } else {
                                        let chr_rect = fz_rect_from_quad((*chr).quad);
                                        rect = fz_union_rect(rect, chr_rect);
                                        text.push(c);
                                    }
                                }
                                chr = (*chr).next;
                            }

                            if !text.is_empty() {
                                words.push(BoundedText {
                                    text: text.clone(),
                                    rect: rect.into(),
                                    location: TextLocation::Static(self.index, offset),
                                });
                                text.clear();
                                rect = FzRect::default();
                                offset += 1;
                            }
                        }

                        line = (*line).next;
                    }
                }

                block = (*block).next;
            }

            fz_drop_stext_page(self.ctx.0, tp);
            Some(words)
        }
    }

    pub fn links(&self) -> Option<Vec<BoundedText>> {
        unsafe {
            let links = mp_load_links(self.ctx.0, self.page);

            if links.is_null() {
                return None;
            }

            let mut link = links;
            let mut result = Vec::new();
            let mut offset = 0;

            while !link.is_null() {
                let text = CStr::from_ptr((*link).uri).to_string_lossy().into_owned();
                let rect = (*link).rect.into();
                result.push(BoundedText {
                    text,
                    rect,
                    location: TextLocation::Static(self.index, offset),
                });
                link = (*link).next;
                offset += 1;
            }

            fz_drop_link(self.ctx.0, links);

            Some(result)
        }
    }

    pub fn pixmap(&self, scale: f32) -> Option<Pixmap> {
        unsafe {
            let mat = fz_scale(scale as libc::c_float, scale as libc::c_float);
            let pixmap = mp_new_pixmap_from_page(self.ctx.0,
                                                 self.page,
                                                 mat,
                                                 fz_device_gray(self.ctx.0),
                                                 0);
            if pixmap.is_null() {
                return None;
            }

            let width = (*pixmap).w as u32;
            let height = (*pixmap).h as u32;
            let len = (width * height) as usize;
            let samples = slice::from_raw_parts((*pixmap).samples, len);
            let mut data = Vec::new();
            if data.try_reserve(len).is_err() {
                fz_drop_pixmap(self.ctx.0, pixmap);
                return None;
            }
            data.extend(samples);

            fz_drop_pixmap(self.ctx.0, pixmap);

            Some(Pixmap { width, height, data })
        }
    }

    pub fn boundary_box(&self) -> Option<Boundary> {
        unsafe {
            let mut rect = FzRect::default();
            let dev = fz_new_bbox_device(self.ctx.0, &mut rect);
            if dev.is_null() {
                None
            } else {
                fz_run_page(self.ctx.0, self.page, dev, fz_identity, ptr::null_mut());
                fz_close_device(self.ctx.0, dev);
                fz_drop_device(self.ctx.0, dev);
                Some(rect.into())
            }
        }
    }

    pub fn dims(&self) -> (f32, f32) {
        unsafe {
            let bounds = fz_bound_page(self.ctx.0, self.page);
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
        unsafe { fz_drop_context(self.0); }
    }
}

impl Drop for PdfDocument {
    fn drop(&mut self) {
        unsafe { fz_drop_document(self.ctx.0, self.doc); }
    }
}

impl<'a> Drop for PdfPage<'a> {
    fn drop(&mut self) {
        unsafe { fz_drop_page(self.ctx.0, self.page); }
    }
}
