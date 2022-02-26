use crate::device::CURRENT_DEVICE;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::color::{BLACK, WHITE};
use crate::gesture::GestureEvent;
use crate::geom::{Rectangle};
use crate::framebuffer::{Framebuffer, UpdateMode};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, ViewId};
use crate::app::Context;

pub struct ChapterLabel {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    title: String,
    progress: f32,
}

impl ChapterLabel {
    pub fn new(rect: Rectangle, title: String, progress: f32)  -> ChapterLabel {
        ChapterLabel {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            title,
            progress,
        }
    }

    pub fn update(&mut self, title: String, progress: f32, rq: &mut RenderQueue) {
        let mut render = false;
        if self.title != title {
            self.title = title;
            render = true;
        }
        if self.progress != progress {
            self.progress = progress;
            render = true;
        }
        if render {
            rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
        }
    }
}


impl View for ChapterLabel {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                bus.push_back(Event::Show(ViewId::TableOfContents));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        fb.draw_rectangle(&self.rect, WHITE);
        if !self.title.is_empty() {
            let dpi = CURRENT_DEVICE.dpi;
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            let padding = font.em() as i32 / 2;
            let max_width = self.rect.width().saturating_sub(2 * padding as u32) as i32;
            let max_progress_width = max_width - font.ellipsis.width;
            let progress_plan = font.plan(&format!(" ({:.1}%)", 100.0 * self.progress),
                                          Some(max_progress_width),
                                          None);
            let max_title_width = max_width - progress_plan.width;
            let title_plan = font.plan(&self.title,
                                       Some(max_title_width),
                                       None);
            let dx = padding + (max_width - title_plan.width - progress_plan.width) / 2;
            let dy = (self.rect.height() as i32 - font.x_heights.0 as i32) / 2;
            let mut pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
            font.render(fb, BLACK, &title_plan, pt);
            pt.x += title_plan.width;
            font.render(fb, BLACK, &progress_plan, pt);
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
