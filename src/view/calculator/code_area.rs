use crate::device::CURRENT_DEVICE;
use crate::font::Fonts;
use crate::input::{DeviceEvent, ButtonCode, ButtonStatus};
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue};
use super::{Line, LineOrigin};
use crate::gesture::GestureEvent;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::unit::mm_to_px;
use crate::geom::{Rectangle, Dir, CycleDir};
use crate::color::TEXT_NORMAL;
use crate::app::Context;

pub struct CodeArea {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    data: Vec<Line>,
    font_size: f32,
    margin_width: i32,
}

impl CodeArea {
    pub fn new(rect: Rectangle, font_size: f32, margin_width: i32) -> CodeArea {
        CodeArea {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            data: Vec::new(),
            font_size,
            margin_width,
        }
    }

    pub fn append(&mut self, line: Line, added_lines: i32, screen_lines: i32, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = &mut context.fonts.monospace.regular;
        font.set_size((64.0 * self.font_size) as u32, dpi);
        let line_height = font.ascender() - font.descender();
        let margin_width_px = mm_to_px(self.margin_width as f32, dpi) as i32;
        let min_y = self.rect.min.y + margin_width_px + screen_lines * line_height;

        let rect = rect![self.rect.min.x + margin_width_px,
                         min_y,
                         self.rect.max.x - margin_width_px,
                         min_y + added_lines * line_height];
        self.data.push(line);
        self.render(context.fb.as_mut(), rect, &mut context.fonts);
        context.fb.update(&rect, UpdateMode::Gui).ok();
    }

    pub fn set_data(&mut self, data: Vec<Line>, context: &mut Context) {
        self.data = data;
        self.render(context.fb.as_mut(), self.rect, &mut context.fonts);
        context.fb.update(&self.rect, UpdateMode::Gui).ok();
    }

    pub fn update(&mut self, font_size: f32, margin_width: i32) {
        self.font_size = font_size;
        self.margin_width = margin_width;
    }
}

impl View for CodeArea {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, end, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::South | Dir::North => bus.push_back(Event::Scroll(start.y - end.y)),
                    Dir::West => bus.push_back(Event::Page(CycleDir::Next)),
                    Dir::East => bus.push_back(Event::Page(CycleDir::Previous)),
                }
                true
            },
            Event::Device(DeviceEvent::Button { code, status: ButtonStatus::Pressed, .. }) => {
                match code {
                    ButtonCode::Backward => bus.push_back(Event::Page(CycleDir::Previous)),
                    ButtonCode::Forward => bus.push_back(Event::Page(CycleDir::Next)),
                    _ => (),
                }
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                let middle_x = (self.rect.min.x + self.rect.max.x) / 2;
                if center.x < middle_x {
                    bus.push_back(Event::Page(CycleDir::Previous));
                } else {
                    bus.push_back(Event::Page(CycleDir::Next));
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        if let Some(irect) = self.rect.intersection(&rect) {
            fb.draw_rectangle(&irect, TEXT_NORMAL[0]);
        }

        let font = &mut fonts.monospace.regular;
        font.set_size((64.0 * self.font_size) as u32, dpi);
        let line_height = font.ascender() - font.descender();
        let char_width = font.plan(" ", None, None).width;
        let padding = mm_to_px(self.margin_width as f32, dpi) as i32;

        let mut x = self.rect.min.x + padding;
        let mut y = self.rect.min.y + padding + font.ascender();

        for line in &self.data {
            let font = match line.origin {
                LineOrigin::Input => &mut fonts.monospace.bold,
                LineOrigin::Output => &mut fonts.monospace.regular,
                LineOrigin::Error => &mut fonts.monospace.italic,
            };

            font.set_size((64.0 * self.font_size) as u32, dpi);

            for c in line.content.chars() {
                if x > self.rect.max.x - padding - char_width {
                    y += line_height;
                    x = self.rect.min.x + padding;
                }
                if y >= rect.min.y {
                    let plan = font.plan(&c.to_string(), None, None);
                    font.render(fb, TEXT_NORMAL[1], &plan, pt!(x, y));
                }
                x += char_width;
            }

            y += line_height;
            x = self.rect.min.x + padding;
        }
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect)
            .unwrap_or(self.rect)
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
