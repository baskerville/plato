use super::{Bus, Event, Filler, Fonts, Hub, Id, RenderQueue, View};
use crate::context::Context;
use crate::framebuffer::Framebuffer;
use crate::geom::Rectangle;

use log::debug;

use super::chess_cell::ChessCell;
use chess::{Color, Piece, Square};
use chess::{ALL_SQUARES, NUM_FILES, NUM_RANKS};
const PADDING_RATIO: f32 = 0.06;

pub struct ChessBoard {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    reversed: bool,
}

impl ChessBoard {
    pub fn new(rect: Rectangle) -> ChessBoard {
        use crate::color::CHESSBOARD_BG;
        use crate::view::ID_FEEDER;

        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let sizes = Self::compute_sizes(rect, false);

        let filler = Filler::new(sizes[0], CHESSBOARD_BG);
        children.push(Box::new(filler) as Box<dyn View>);
        for square in ALL_SQUARES {
            let cell = ChessCell::new(sizes[square.to_index() + 1], square);
            children.push(Box::new(cell) as Box<dyn View>);
        }

        ChessBoard {
            id,
            rect,
            children,
            reversed: false,
        }
    }

    fn compute_sizes(rect: Rectangle, reversed: bool) -> [Rectangle; 1 + NUM_FILES * NUM_RANKS] {
        let mut sizes = [rect; 1 + NUM_FILES * NUM_RANKS];

        let cell_height = (rect.height() / NUM_RANKS as u32).min(rect.width() / NUM_FILES as u32) as i32;
        let cell_width = cell_height;
        let padding = PADDING_RATIO as i32 * cell_height;

        // background
        sizes[0] = rect;

        let start_y = rect.max.y - (rect.height() as i32 - (cell_height * NUM_RANKS as i32)) / 2 - padding;
        let start_x = rect.min.x + (rect.width() as i32 - (cell_width * NUM_FILES as i32)) / 2 + padding;
        for square in ALL_SQUARES {
            let rank = if !reversed {
                square.get_rank().to_index() as i32
            } else {
                7 - square.get_rank().to_index() as i32
            };
            let file = if !reversed {
                square.get_file().to_index() as i32
            } else {
                7 - square.get_file().to_index() as i32
            };
            let y = start_y - rank * (padding + cell_height) - cell_height;
            let x = start_x + file * (padding + cell_width);

            sizes[square.to_index() + 1] = rect![x, y, x + cell_width, y + cell_height];
        }

        sizes
    }

    pub fn reverse(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.reversed = !self.reversed;
        self.resize(self.rect, hub, rq, context);
    }

    pub fn set_active_square(&mut self, rq: &mut RenderQueue, square: Square, active: bool) {
        if let Some(cell) = self.child_mut(square.to_index() + 1).downcast_mut::<ChessCell>() {
            if active {
                cell.active(rq);
            } else {
                cell.release(rq);
            }
        }
    }

    pub fn update_square(&mut self, rq: &mut RenderQueue, square: Square, piece: Option<Piece>, color: Option<Color>) {
        if let Some(cell) = self.child_mut(square.to_index() + 1).downcast_mut::<ChessCell>() {
            cell.update(rq, piece, color);
        }
    }
}

impl View for ChessBoard {
    fn handle_event(
        &mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context,
    ) -> bool {
        use crate::gesture::GestureEvent;
        use crate::input::DeviceEvent;

        match *evt {
            Event::Gesture(GestureEvent::Swipe { start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn might_skip(&self, evt: &Event) -> bool {
        use crate::input::DeviceEvent;

        !matches!(
            *evt,
            Event::Key(..) | Event::Gesture(..) | Event::Device(DeviceEvent::Finger { .. }) | Event::Select(..)
        )
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        debug!("Resizing chess board from {} to {}", self.rect, rect);
        self.rect = rect;

        let sizes = Self::compute_sizes(rect, self.reversed);
        for (index, size) in sizes.iter().enumerate() {
            self.children[index].resize(*size, hub, rq, context);
        }
    }

    fn is_background(&self) -> bool {
        true
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
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
