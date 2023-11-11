use crate::view::icon::Icon;
use crate::view::key::KeyKind;
use crate::view::label::Label;
use crate::view::{Align, Bus, Event, Hub, Id, RenderData, RenderQueue, View, ID_FEEDER};

use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::{font_from_style, Fonts, NORMAL_STYLE};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::Rectangle;
use crate::gesture::GestureEvent;
use crate::input::DeviceEvent;

use log::debug;

use chess::Color;

use std::time::Duration;

#[derive(Debug)]
pub struct BottomBar {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    color: Color,
    human: bool,
    clocks: [Duration; 2],
}

impl BottomBar {
    pub fn new(
        rect: Rectangle, name: &str, color: Color, human: bool, context: &mut Context,
    ) -> BottomBar {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();

        let sizes = Self::compute_sizes(rect, context);

        let icon = Icon::new(
            match color {
                Color::White => "wchess",
                Color::Black => "bchess",
            },
            sizes[0],
            Event::Key(KeyKind::Alternate),
        );
        children.push(Box::new(icon) as Box<dyn View>);

        let icon = Icon::new("arrow-left", sizes[1], Event::Cancel);
        children.push(Box::new(icon) as Box<dyn View>);

        let name_label = Label::new(sizes[2], name.to_string(), Align::Center);
        children.push(Box::new(name_label) as Box<dyn View>);

        let mut white_time_label = Label::new(sizes[3], "––:––:––".to_string(), Align::Left(0));
        white_time_label.set_font_size(context.settings.chess_engine_settings.font_size);
        children.push(Box::new(white_time_label) as Box<dyn View>);
        let mut black_time_label = Label::new(sizes[4], "––:––:––".to_string(), Align::Right(0));
        black_time_label.set_font_size(context.settings.chess_engine_settings.font_size);
        children.push(Box::new(black_time_label) as Box<dyn View>);

        let icon = Icon::new(
            if human { "human-chess" } else { "engine-chess" },
            sizes[5],
            Event::ChessGo(human),
        );
        children.push(Box::new(icon) as Box<dyn View>);

        BottomBar {
            id,
            rect,
            children,
            color,
            human,
            clocks: [Duration::new(0, 0), Duration::new(0, 0)],
        }
    }

    fn compute_sizes(rect: Rectangle, context: &mut Context) -> [Rectangle; 6] {
        let side = rect.height() as i32;

        let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, CURRENT_DEVICE.dpi);
        let plan = font.plan("––:––:–– ←", None, None);
        let time_width = 15 * plan.width / 10;

        [
            rect![rect.min, rect.min + side],
            // undo_icon
            rect![
                pt!(rect.min.x + side, rect.min.y),
                pt!(rect.min.x + 2 * side, rect.min.y + side)
            ],
            // labels
            rect![
                pt!(rect.min.x + 2 * side, rect.min.y),
                pt!(rect.max.x - side - time_width, rect.max.y)
            ],
            rect![
                pt!(rect.max.x - side - time_width, rect.min.y),
                pt!(rect.max.x - side, rect.max.y - rect.height() as i32 / 2)
            ],
            rect![
                pt!(rect.max.x - side - time_width, rect.min.y + rect.height() as i32 / 2),
                pt!(rect.max.x - side, rect.max.y)
            ],
            rect![rect.max - side, rect.max],
        ]
    }

    pub fn update_color(&mut self, color: Color, rq: &mut RenderQueue) {
        if self.color != color {
            let index = 0;
            let color_rect = *self.child(index).rect();

            let icon = Icon::new(
                match color {
                    Color::White => "wchess",
                    Color::Black => "bchess",
                },
                color_rect,
                Event::Key(KeyKind::Alternate),
            );
            rq.add(RenderData::new(icon.id(), color_rect, UpdateMode::Gui));
            self.children[index] = Box::new(icon) as Box<dyn View>;
        }
        self.color = color;

        self.update_clocks(self.clocks[0], self.clocks[1], rq)
    }

    pub fn update_player(&mut self, human: bool, rq: &mut RenderQueue) {
        if human != self.human {
            let index = self.len() - 1;
            let rect = *self.child(index).rect();

            let icon = Icon::new(
                if human { "human-chess" } else { "engine-chess" },
                rect,
                Event::ChessGo(human),
            );
            rq.add(RenderData::new(icon.id(), rect, UpdateMode::Gui));
            self.children[index] = Box::new(icon) as Box<dyn View>;
        }
        self.human = human;
    }

    pub fn update_name(&mut self, text: &str, rq: &mut RenderQueue) {
        let name_label = self.child_mut(2).downcast_mut::<Label>().unwrap();
        name_label.update(text, rq);
    }

    pub fn update_clocks(&mut self, white_clock: Duration, black_clock: Duration, rq: &mut RenderQueue) {

        self.clocks = [white_clock, black_clock];
        let mark = if self.color == Color::White { "←" } else { "" };
        let text =
            if white_clock == Duration::new(0,0) {
                format!("#:##:## {}", mark)
            } else {
                let h = white_clock.as_secs() / 3600;
                let m = (white_clock.as_secs() - h * 3600) / 60;
                let s = white_clock.as_secs() - (h * 3600 + m * 60);
                format!("{}:{:02}:{:02} {}", h, m, s, mark)
            };
        self.child_mut(3).downcast_mut::<Label>()
            .unwrap()
            .update(&text, rq);

        let mark = if self.color == Color::Black { "→" } else { "" };
        let text =
            if black_clock == Duration::new(0,0) {
                format!("{} #:##:##", mark)
            } else {
                let h = black_clock.as_secs() / 3600;
                let m = (black_clock.as_secs() - h * 3600) / 60;
                let s = black_clock.as_secs() - (h * 3600 + m * 60);
                format!("{} {}:{:02}:{:02}", mark, h, m, s)
            };
        self.child_mut(4).downcast_mut::<Label>()
            .unwrap()
            .update(&text, rq);
    }
}

impl View for BottomBar {
    fn handle_event(
        &mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) | Event::Gesture(GestureEvent::HoldFingerShort(center, ..))
                if self.rect.includes(center) =>
            {
                true
            }
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {}

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        debug!("Resizing bottom bar from {} to {}", self.rect, rect);
        self.rect = rect;

        let sizes = Self::compute_sizes(rect, context);
        for (index, size) in sizes.iter().enumerate() {
            self.children[index].resize(*size, hub, rq, context);
        }
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
