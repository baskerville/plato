use crate::color::{TEXT_INVERTED_HARD, TEXT_NORMAL};
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::{font_from_style, Fonts};
use crate::font::{MD_AUTHOR, MD_TITLE};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{halves, Rectangle};
use crate::gesture::GestureEvent;
use crate::opds::{Entry};
use crate::view::{
    Bus, EntryId, Event, Hub, Id, RenderData, RenderQueue, View, ID_FEEDER,
};

pub struct FeedEntry {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    entry: Entry,
    active: bool,
}

impl FeedEntry {
    pub fn new(rect: Rectangle, entry: Entry) -> FeedEntry {
        FeedEntry {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            entry,
            active: false,
        }
    }
}

impl View for FeedEntry {
    fn handle_event(
        &mut self,
        evt: &Event,
        hub: &Hub,
        _bus: &mut Bus,
        rq: &mut RenderQueue,
        _context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                self.active = true;
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                hub.send(Event::Select(EntryId::OpdsEntry(self.entry.id.clone()))).ok();
                true
            }
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let scheme = if self.active {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        fb.draw_rectangle(&self.rect, scheme[0]);

        let title = self.entry.title.as_str();

        let (x_height, padding, baseline) = {
            let font = font_from_style(fonts, &MD_TITLE, dpi);
            let x_height = font.x_heights.0 as i32;
            (
                x_height,
                font.em() as i32,
                (self.rect.height() as i32 - 2 * x_height) / 3,
            )
        };

        let (small_half_padding, _big_half_padding) = halves(padding);
        let third_width = 6 * x_height;
        let second_width = 8 * x_height;
        let first_width = self.rect.width() as i32 - second_width - third_width;
        let width = first_width - padding - small_half_padding;
        let start_x = self.rect.min.x + padding;

        // Author
        let mut author_width = 0;
        if self.entry.author.is_some() {
            author_width = {
                let font = font_from_style(fonts, &MD_AUTHOR, dpi);
                let plan = font.plan(self.entry.author.as_deref().unwrap(), Some(width), None);
                let pt = pt!(start_x, self.rect.max.y - baseline);
                font.render(fb, scheme[1], &plan, pt);
                plan.width
            };
        }

        // Title
        {
            let font = font_from_style(fonts, &MD_TITLE, dpi);
            let mut plan = font.plan(&title, None, None);
            let mut title_lines = 1;

            if plan.width > width {
                let available = width - author_width;
                if available > 3 * padding {
                    let (index, usable_width) = font.cut_point(&plan, width);
                    let leftover = plan.width - usable_width;
                    if leftover > 2 * padding {
                        let mut plan2 = plan.split_off(index, usable_width);
                        let max_width = available - if author_width > 0 { padding } else { 0 };
                        font.trim_left(&mut plan2);
                        font.crop_right(&mut plan2, max_width);
                        let pt = pt!(
                            self.rect.min.x + first_width - small_half_padding - plan2.width,
                            self.rect.max.y - baseline
                        );
                        font.render(fb, scheme[1], &plan2, pt);
                        title_lines += 1;
                    } else {
                        font.crop_right(&mut plan, width);
                    }
                } else {
                    font.crop_right(&mut plan, width);
                }
            }

            let dy = if author_width == 0 && title_lines == 1 {
                (self.rect.height() as i32 - x_height) / 2 + x_height
            } else {
                baseline + x_height
            };

            //TODO add type

            let pt = pt!(start_x, self.rect.min.y + dy);
            font.render(fb, scheme[1], &plan, pt);
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
