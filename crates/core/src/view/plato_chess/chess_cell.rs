use crate::framebuffer::{Framebuffer, UpdateMode};

use super::{Bus, Event, Hub, Id, RenderData, RenderQueue, View};
use crate::context::Context;
use crate::font::Fonts;
use crate::geom::Rectangle;

use chess::{Color, Piece, Square};

use crate::device::CURRENT_DEVICE;
use crate::document::pdf::PdfOpener;
use crate::framebuffer::Pixmap;
use crate::unit::scale_by_dpi_raw;
use fxhash::FxHashMap;
use lazy_static::lazy_static;
use std::path::Path;

use log::debug;

const ICON_SCALE: f32 = 1.0 / 32.0;
const CHESS_ICON_SCALE: f32 = ICON_SCALE * 80.0;
lazy_static! {
    pub static ref CHESS_PIXMAPS: FxHashMap<&'static str, Pixmap> = {
        let mut m = FxHashMap::default();
        let scale = scale_by_dpi_raw(CHESS_ICON_SCALE, CURRENT_DEVICE.dpi);
        let dir = Path::new("icons").join("chess");
        for name in ["bB", "bK", "bN", "bP", "bQ", "bR"].iter().cloned() {
            let path = dir.join(&format!("{}.svg", name));
            let doc = PdfOpener::new()
                .and_then(|o| o.open(&path))
                .unwrap_or_else(|| panic!("Pdfopener error on {:?}", &path));
            let pixmap = doc
                .page(0)
                .and_then(|p| p.pixmap(scale))
                .unwrap_or_else(|| panic!("pixmap conversion error on {:?}", &path));
            m.insert(name, pixmap);
        }
        m
    };
}

use std::fmt::Debug;
#[derive(Debug)]
pub struct ChessCell {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    square: Square,
    piece: Option<Piece>,
    color: Option<Color>,
    active: bool,
}

impl ChessCell {
    pub fn new(rect: Rectangle, square: Square) -> ChessCell {
        use crate::view::ID_FEEDER;
        ChessCell {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            square,
            piece: None,
            color: None,
            active: false,
        }
    }

    pub fn update(&mut self, rq: &mut RenderQueue, piece: Option<Piece>, color: Option<Color>) {
        if (piece, color) != (self.piece, self.color) {
            (self.piece, self.color) = (piece, color);
        }
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    pub fn active(&mut self, rq: &mut RenderQueue) {
        self.active = true;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    pub fn release(&mut self, rq: &mut RenderQueue) {
        self.active = false;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }
}

impl View for ChessCell {
    fn handle_event(
        &mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context,
    ) -> bool {
        use crate::gesture::GestureEvent;

        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                bus.push_back(Event::ChessCell(self.square, self.active));
                true
            }
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        use crate::color::{BLACK, CELL_BLACK, CELL_WHITE, WHITE};
        use crate::geom::CornerSpec;
        use crate::unit::scale_by_dpi;
        use crate::view::BORDER_RADIUS_MEDIUM;

        debug!("Render cell {:?} at {}", self.square, self.rect);

        let dpi = CURRENT_DEVICE.dpi;
        let scheme: [u8; 2] = if self.square.get_file().to_index() % 2 != self.square.get_rank().to_index() % 2 {
            CELL_WHITE
        } else {
            CELL_BLACK
        };

        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        fb.draw_rounded_rectangle(
            &self.rect,
            &CornerSpec::Uniform(border_radius),
            scheme[if !self.active { 0 } else { 1 }],
        );

        if let (Some(piece), Some(color)) = (self.piece, self.color) {
            let mut img_path = String::new();
            img_path.push('b');
            img_path.push(piece.to_string(Color::White).chars().next().unwrap());

            let pixmap = &CHESS_PIXMAPS[img_path.as_str()];

            let dx = (self.rect.width() as i32 - pixmap.width as i32) / 2;
            let dy = (self.rect.height() as i32 - pixmap.height as i32) / 2;
            let pt = self.rect.min + pt!(dx, dy);
            fb.draw_blended_pixmap(pixmap, pt, if color == Color::White { WHITE } else { BLACK });
        }
    }

    fn resize(&mut self, rect: Rectangle, _hub: &Hub, _rq: &mut RenderQueue, _context: &mut Context) {
        self.rect = rect;
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<dyn View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.children
    }

    fn id(&self) -> Id {
        self.id
    }
}
