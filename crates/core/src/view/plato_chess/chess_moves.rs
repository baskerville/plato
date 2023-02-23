use crate::color::TEXT_NORMAL;
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::{Fonts, RenderPlan};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{CycleDir, Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::unit::mm_to_px;
use crate::view::{Bus, Event, Hub, Id, RenderData, RenderQueue, View, ID_FEEDER};

use chess::{BitBoard, Board, ChessMove, Color, MoveGen, Piece};
use std::fmt;

pub enum MoveType {
    Normal,
    KingCastle,
    QueenCastle,
    Capture,
}
pub struct DetailedChessMove {
    color: Option<Color>,
    piece: Option<Piece>,
    move_type: MoveType,
    data: ChessMove,
    check: bool,
    mate: bool,
}

impl DetailedChessMove {
    pub fn new(position: Board, chessmove: &ChessMove) -> Self {
        let piece = position.piece_on(chessmove.get_source());
        let color = position.color_on(chessmove.get_source());
        let move_type = if position.piece_on(chessmove.get_dest()).is_some() {
            MoveType::Capture
        } else if piece == Some(Piece::King) {
            match (chessmove.get_source().to_index(), chessmove.get_dest().to_index()) {
                (4, 6) => MoveType::KingCastle,
                (4, 2) => MoveType::QueenCastle,
                (60, 62) => MoveType::KingCastle,
                (60, 58) => MoveType::QueenCastle,
                _ => MoveType::Normal,
            }
        } else {
            MoveType::Normal
        };
        let after_move_position = position.make_move_new(*chessmove);
        let mate = MoveGen::new_legal(&after_move_position).into_iter().len() == 0;
        let check = *after_move_position.checkers() != BitBoard::new(0);
        Self {
            color,
            piece,
            move_type,
            data: *chessmove,
            check,
            mate,
        }
    }
}

impl fmt::Display for DetailedChessMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let pieces = [['♟', '♞', '♝', '♜', '♛', '♚'], ['♙', '♘', '♗', '♖', '♕', '♔']];
        let piece = match (self.color, self.piece) {
            (Some(Color::Black), Some(p)) => pieces[0][p.to_index()],
            (Some(Color::White), Some(p)) => pieces[1][p.to_index()],
            _ => '?',
        };
        let promotion = match self.data.get_promotion() {
            Some(p) => format!(
                "={}",
                match self.color {
                    Some(Color::Black) => pieces[0][p.to_index()],
                    Some(Color::White) => pieces[1][p.to_index()],
                    _ => '?',
                }
            ),
            None => "".to_string(),
        };
        let check = if self.mate {
            "#"
        } else if self.check {
            "+"
        } else {
            ""
        };
        match self.move_type {
            MoveType::Normal => write!(
                f,
                "{}{}{}{}{}",
                piece,
                self.data.get_source(),
                self.data.get_dest(),
                promotion,
                check
            ),
            MoveType::Capture => write!(
                f,
                "{}{}x{}{}{}",
                piece,
                self.data.get_source(),
                self.data.get_dest(),
                promotion,
                check
            ),
            MoveType::KingCastle => write!(f, "0-0"),
            MoveType::QueenCastle => write!(f, "0-0-0"),
        }
    }
}

pub struct MovesArea {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    data_string: Vec<String>,
    plans: Vec<RenderPlan>,
    last_plan_min_element: usize,
    pos: usize,
    font_size: f32,
    margin_width: i32,
}

impl MovesArea {
    pub fn new(rect: Rectangle, font_size: f32, margin_width: i32, _context: &mut Context) -> MovesArea {
        MovesArea {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            data_string: Vec::new(),
            plans: Vec::new(),
            last_plan_min_element: 0,
            pos: 0,
            font_size,
            margin_width,
        }
    }

    pub fn pop(&mut self, rq: &mut RenderQueue, context: &mut Context) -> Option<String> {
        let move_string = self.data_string.pop();
        self.update_plan(context);
        rq.add(RenderData::new(self.id(), self.rect, UpdateMode::Gui));
        move_string
    }

    pub fn append(&mut self, chessmove: &DetailedChessMove, rq: &mut RenderQueue, context: &mut Context) {
        let i = self.data_string.len() as isize;
        let num = if (i % 2) == 0 {
            format!(" {}.", i / 2 + 1)
        } else {
            "".to_string()
        };
        let content = format!("{} {}", num, &chessmove);
        self.data_string.push(content);

        self.update_partial_plan(context);
        rq.add(RenderData::new(self.id(), self.rect, UpdateMode::Gui));
    }

    pub fn clear(&mut self, rq: &mut RenderQueue) {
        self.pos = 0;
        self.last_plan_min_element = 0;
        self.data_string.clear();
        self.plans.clear();
        rq.add(RenderData::new(self.id(), self.rect, UpdateMode::Gui));
    }

    fn update_plan(&mut self, context: &mut Context) {
        let font = &mut context.fonts.display;
        let dpi = CURRENT_DEVICE.dpi;
        font.set_size((64.0 * self.font_size) as u32, dpi);
        let padding = mm_to_px(self.margin_width as f32, dpi) as i32;

        let pos_at_end = self.pos == self.plans.len();
        self.plans.clear();
        let mut min_element = 0;
        let mut max_element = self.data_string.len();
        loop {
            let plan = font.plan(self.data_string[min_element..max_element].concat(), None, None);
            let x = self.rect.min.x + padding + plan.width;
            if x < self.rect.max.x - padding && max_element >= self.data_string.len() {
                self.plans.push(plan);
                break;
            } else if x < self.rect.max.x {
                self.plans.push(plan);
                min_element = max_element;
                max_element = self.data_string.len();
            } else if max_element == 0 {
                break;
            } else {
                max_element -= 1;
            }
        }
        self.last_plan_min_element = min_element;
        if pos_at_end {
            self.pos = self.plans.len();
        }
    }

    fn update_partial_plan(&mut self, context: &mut Context) {
        let font = &mut context.fonts.display;
        let dpi = CURRENT_DEVICE.dpi;
        font.set_size((64.0 * self.font_size) as u32, dpi);
        let padding = mm_to_px(self.margin_width as f32, dpi) as i32;

        let pos_at_end = self.pos == self.plans.len();

        let mut min_element = self.last_plan_min_element;
        let mut max_element = self.data_string.len();
        // remove and reconstruct last line
        self.plans.pop();
        loop {
            let plan = font.plan(self.data_string[min_element..max_element].concat(), None, None);
            let x = self.rect.min.x + padding + plan.width;
            if x < self.rect.max.x - padding && max_element >= self.data_string.len() {
                self.plans.push(plan);
                break;
            } else if x < self.rect.max.x {
                self.plans.push(plan);
                min_element = max_element;
                max_element = self.data_string.len();
            } else if max_element == 0 {
                break;
            } else {
                max_element -= 1;
            }
        }
        self.last_plan_min_element = min_element;
        if pos_at_end {
            self.pos = self.plans.len();
        }
    }
}

impl View for MovesArea {
    fn handle_event(
        &mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, end, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::South | Dir::North => {
                        let dpi = CURRENT_DEVICE.dpi as i32;
                        let min = self.plans.len().min(1);
                        let max = self.plans.len();
                        let absolute_pos = 0.max(self.pos as i32 - (end.y - start.y) / (dpi / 3)) as usize;
                        self.pos = min.max(max.min(absolute_pos));
                    }
                    Dir::West => self.pos = self.plans.len().min(1),
                    Dir::East => self.pos = self.plans.len(),
                }
                rq.add(RenderData::new(self.id(), self.rect, UpdateMode::Gui));
                true
            }
            Event::Device(DeviceEvent::Button {
                code,
                status: ButtonStatus::Pressed,
                ..
            }) => {
                match code {
                    ButtonCode::Backward => bus.push_back(Event::Page(CycleDir::Previous)),
                    ButtonCode::Forward => bus.push_back(Event::Page(CycleDir::Next)),
                    _ => (),
                }
                true
            }
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                let middle_x = (self.rect.min.x + self.rect.max.x) / 2;
                if center.x < middle_x {
                    bus.push_back(Event::Page(CycleDir::Previous));
                } else {
                    bus.push_back(Event::Page(CycleDir::Next));
                }
                true
            }
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = &mut fonts.display;
        let line_height = font.ascender() - font.descender();
        let padding = mm_to_px(self.margin_width as f32, dpi) as i32;

        fb.draw_rectangle(&self.rect, TEXT_NORMAL[0]);

        let x = self.rect.min.x + padding;
        let mut y = self.rect.max.y - padding;

        for plan in self.plans[..self.pos].iter().rev() {
            if y < self.rect.min.y + padding {
                break;
            }
            font.render(fb, TEXT_NORMAL[1], plan, pt!(x, y));
            y -= line_height;
        }
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect).unwrap_or(self.rect)
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
