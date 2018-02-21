extern crate libc;

use document::mupdf_sys::*;

use std::ptr;
use std::slice;
use std::char;
use std::rc::Rc;
use std::path::Path;
use std::io::Read;
use std::fs::File;
use std::ffi::{CString, CStr};
use std::os::unix::ffi::OsStrExt;
use document::{Document, TextLayer, LayerGrain, TocEntry, Link};
use framebuffer::Pixmap;
use geom::Rectangle;

error_chain!{
    foreign_links {
        Io(::std::io::Error);
        NulError(::std::ffi::NulError);
    }
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

struct PdfContext(*mut FzContext);

pub struct PdfOpener(Rc<PdfContext>);

pub struct PdfDocument {
    ctx: Rc<PdfContext>,
    doc: *mut FzDocument,
}

pub struct PdfPage<'a> {
    ctx: Rc<PdfContext>,
    page: *mut FzPage,
    doc: &'a PdfDocument,
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
                    doc: doc,
                })
            }
        }
    }

    pub fn open_memory(&self, kind: &str, buf: &[u8]) -> Option<PdfDocument> {
        unsafe {
            let stream = fz_open_memory((self.0).0, buf.as_ptr() as *const libc::c_uchar, buf.len() as libc::size_t);
            let c_kind = CString::new(kind).unwrap();
            let doc = mp_open_document_with_stream((self.0).0, c_kind.as_ptr(), stream);
            fz_drop_stream((self.0).0, stream);
            if doc.is_null() {
                None
            } else {
                Some(PdfDocument {
                    ctx: self.0.clone(),
                    doc: doc,
                })
            }
        }
    }

    pub fn set_use_document_css(&mut self, should_use: bool) {
        unsafe {
            fz_set_use_document_css((self.0).0, should_use as libc::c_int);
        }
    }

    pub fn set_user_css<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
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

impl PdfDocument {
    pub fn page(&self, index: usize) -> Option<PdfPage> {
        unsafe {
            let page = mp_load_page(self.ctx.0, self.doc, index as libc::c_int);
            if page.is_null() {
                None
            } else {
                Some(PdfPage {
                    ctx: self.ctx.clone(),
                    page: page,
                    doc: self,
                })
            }
        }
    }

    fn walk_toc(outline: *mut FzOutline) -> Vec<TocEntry> {
        unsafe {
            let mut vec = Vec::new();
            let mut cur = outline;
            while !cur.is_null() {
                let title = CStr::from_ptr((*cur).title).to_string_lossy().into_owned();
                // TODO: handle page == -1
                let page = (*cur).page as usize;
                let children = if !(*cur).down.is_null() {
                    Self::walk_toc((*cur).down)
                } else {
                    Vec::new()
                };
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

    pub fn info(&self, key: &str) -> Option<String> {
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

    pub fn is_protected(&self) -> bool {
        unsafe { fz_needs_password(self.ctx.0, self.doc) == 1 }
    }
}

impl Document for PdfDocument {
    fn pages_count(&self) -> usize {
        unsafe {
            let count = mp_count_pages(self.ctx.0, self.doc);
            if count < 0 {
                0
            } else {
                count as usize
            }
        }
    }

    fn pixmap(&self, index: usize, scale: f32) -> Option<Pixmap> {
        self.page(index).and_then(|p| p.pixmap(scale))
    }

    fn dims(&self, index: usize) -> Option<(f32, f32)> {
        self.page(index).map(|page| page.dims())
    }

    fn toc(&self) -> Option<Vec<TocEntry>> {
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

    fn text(&self, index: usize) -> Option<TextLayer> {
        self.page(index).and_then(|page| page.text())
    }

    fn links(&self, index: usize) -> Option<Vec<Link>> {
        self.page(index).and_then(|page| page.links())
    }

    fn title(&self) -> Option<String> {
        self.info(FZ_META_INFO_TITLE)
    }

    fn author(&self) -> Option<String> {
        self.info(FZ_META_INFO_AUTHOR)
    }

    fn is_reflowable(&self) -> bool {
        unsafe { fz_is_document_reflowable(self.ctx.0, self.doc) == 1 }
    }

    fn layout(&mut self, width: f32, height: f32, em: f32) {
        unsafe {
            fz_layout_document(self.ctx.0, self.doc,
                               width as libc::c_float,
                               height as libc::c_float,
                               em as libc::c_float);
        }
    }
}

impl<'a> PdfPage<'a> {
    pub fn text(&self) -> Option<TextLayer> {
        unsafe {
            let tp = mp_new_stext_page_from_page(self.ctx.0, self.page, ptr::null());
            if tp.is_null() {
                return None;
            }
            let mut text_page = TextLayer {
                grain: LayerGrain::Page,
                rect: Rectangle::default(),
                children: vec![],
                text: None,
            };
            let mut page_rect = FzRect::default();
            let mut block = (*tp).first_block;
            while !block.is_null() {
                fz_union_rect(&mut page_rect, &(*block).bbox);
                if (*block).kind == FZ_PAGE_BLOCK_TEXT {
                    let text_block = (*block).u.text;
                    let mut line = text_block.first_line;
                    while !line.is_null() {
                        let mut chr = (*line).first_char;
                        let mut text_line = TextLayer {
                            grain: LayerGrain::Line,
                            rect: (*line).bbox.clone().into(),
                            children: vec![],
                            text: None,
                        };
                        let mut word = String::default();
                        let mut word_rect = FzRect::default();
                        while !chr.is_null() {
                            while !chr.is_null() {
                                if let Some(c) = char::from_u32((*chr).c as u32) {
                                    if c.is_whitespace() {
                                        chr = (*chr).next;
                                        break;
                                    } else {
                                        fz_union_rect(&mut word_rect, &(*chr).bbox);
                                        word.push(c);
                                    }
                                }
                                chr = (*chr).next;
                            }
                            if !word.is_empty() {
                                text_line.children.push(
                                    TextLayer {
                                        grain: LayerGrain::Word,
                                        rect: word_rect.clone().into(),
                                        children: vec![],
                                        text: Some(word.clone()),
                                    }
                                );
                                word.clear();
                                word_rect = FzRect::default();
                            }
                        }
                        if !text_line.children.is_empty() {
                            text_page.children.push(text_line);
                        }
                        line = (*line).next;
                    }
                }
                block = (*block).next;
            }
            text_page.rect = page_rect.into();
            fz_drop_stext_page(self.ctx.0, tp);
            Some(text_page)
        }
    }

    pub fn links(&self) -> Option<Vec<Link>> {
        unsafe {
            let links = fz_load_links(self.ctx.0, self.page);

            if links.is_null() {
                return None;
            }

            let mut link = links;
            let mut result = Vec::new();

            while !link.is_null() {
                let uri = CStr::from_ptr((*link).uri).to_string_lossy().into_owned();
                let rect = (*link).rect.clone().into();
                result.push(Link { uri, rect });
                link = (*link).next;
            }

            fz_drop_link(self.ctx.0, links);

            Some(result)
        }
    }

    pub fn pixmap(&self, scale: f32) -> Option<Pixmap> {
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

            let width = (*pixmap).w;
            let height = (*pixmap).h;
            let len = (width * height) as usize;
            let buf = slice::from_raw_parts((*pixmap).samples, len).to_vec();

            fz_drop_pixmap(self.ctx.0, pixmap);

            Some(Pixmap { buf, width, height })
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
