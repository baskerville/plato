use std::path::Path;
use device::CURRENT_DEVICE;
use framebuffer::{Framebuffer, UpdateMode};
use view::{View, Event, ChildEvent};
use gesture::GestureEvent;
use input::{FingerStatus, DeviceEvent};
use document::pdf::PdfOpener;
use document::HumanSize;
use unit::scale_by_dpi;
use metadata::Info;
use font::{Fonts, font_from_style};
use font::{MD_TITLE, MD_AUTHOR, MD_YEAR, MD_KIND, MD_SIZE};
use geom::{Rectangle, halves};
use color::{TEXT_NORMAL, TEXT_INVERTED};

pub struct Book {
    rect: Rectangle,
    info: Info,
    index: usize,
    active: bool,
}

impl Book {
    pub fn new(rect: Rectangle, info: Info, index: usize) -> Book {
        Book {
            rect,
            info,
            index,
            active: false,
        }
    }
}

impl View for Book {
    fn handle_event(&mut self, evt: &Event, bus: &mut Vec<ChildEvent>) -> bool {
        match *evt {
            Event::GestureEvent(GestureEvent::Relay(de)) => {
                match de {
                    DeviceEvent::Finger { status, ref position, .. } => {
                        match status {
                            FingerStatus::Down if self.rect.includes(position) => {
                                self.active = true;
                                bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                false
                            },
                            FingerStatus::Up => {
                                self.active = false;
                                bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                false
                            },
                            FingerStatus::Motion => {
                                let active = self.rect.includes(position);
                                if active != self.active {
                                    self.active = active;
                                    bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                }
                                false
                            }
                            _ => false,
                        }
                    },
                    _ => false,
                }
            },
            Event::GestureEvent(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => {
                bus.push(ChildEvent::Open(self.index));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let scheme = if self.active {
            TEXT_INVERTED
        } else {
            TEXT_NORMAL
        };
        fb.draw_rectangle(&self.rect, scheme[0]);
        let title = self.info.title();
        let author = &self.info.author;
        let year = &self.info.year;
        let file_info = &self.info.file;
        let (x_height, padding, baseline) = {
            let font = font_from_style(fonts, &MD_TITLE, dpi);
            let x_height = font.x_heights.0 as i32;
            (x_height, font.em() as i32, (self.rect.height() as i32 - 2 * x_height) / 3)
        };
        let (small_half_padding, big_half_padding) = halves(padding);
        let first_width = 3 * self.rect.width() as i32 / 4;
        let second_width = (self.rect.width() as i32 - first_width) / 2;
        let width = first_width - padding - small_half_padding;
        // Author
        let author_width = {
            let font = font_from_style(fonts, &MD_AUTHOR, dpi);
            let plan = font.plan(author, Some(width as u32), None);
            let pt = pt!(self.rect.min.x + padding, self.rect.max.y - baseline);
            font.render(fb, scheme[1], &plan, &pt);
            plan.width as i32
        };
        // Title
        {
            let font = font_from_style(fonts, &MD_TITLE, dpi);
            let mut plan = font.plan(&title, None, None);
            if plan.width > width as u32 {
                let available = width - author_width;
                if available > 3 * padding {
                    let (index, usable_width) = font.last_word_before(&plan, width as u32);
                    let leftover = (plan.width - usable_width) as i32;
                    if leftover > 2 * padding {
                        let mut plan2 = plan.split_off(index, usable_width);
                        let max_width = available - padding;
                        font.crop(&mut plan2, max_width as u32);
                        let pt = pt!(self.rect.min.x + first_width - small_half_padding - plan2.width as i32,
                                     self.rect.max.y - baseline);
                        font.render(fb, scheme[1], &plan2, &pt);
                    } else {
                        font.crop(&mut plan, width as u32);
                    }
                } else {
                    font.crop(&mut plan, width as u32);
                }
            }
            let pt = self.rect.min + pt!(padding, baseline + x_height);
            font.render(fb, scheme[1], &plan, &pt);
        }
        // Year
        {
            let font = font_from_style(fonts, &MD_YEAR, dpi);
            let plan = font.plan(year, None, None);
            let dx = (second_width - padding - plan.width as i32) / 2;
            let dy = (self.rect.height() as i32 - font.x_heights.1 as i32) / 2;
            let pt = pt!(self.rect.min.x + first_width + big_half_padding + dx,
                         self.rect.max.y - dy);
            font.render(fb, scheme[1], &plan, &pt);
        }
        // File kind
        {
            let kind = file_info.kind.to_uppercase();
            let font = font_from_style(fonts, &MD_KIND, dpi);
            let mut plan = font.plan(&kind, None, None);
            let letter_spacing = scale_by_dpi(3.0, dpi).max(1.0) as u32;
            plan.space_out(letter_spacing);
            let pt = pt!(self.rect.max.x - padding - plan.width as i32,
                         self.rect.min.y + baseline + x_height);
            font.render(fb, scheme[1], &plan, &pt);
        }
        // File size
        {
            let size = file_info.size.human_size();
            let font = font_from_style(fonts, &MD_SIZE, dpi);
            let mut plan = font.plan(&size, None, None);
            let pt = pt!(self.rect.max.x - padding - plan.width as i32,
                         self.rect.max.y - baseline);
            font.render(fb, scheme[1], &plan, &pt);
        }
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn len(&self) -> usize {
        0
    }

    fn child(&self, _: usize) -> &View {
        self
    }

    fn child_mut(&mut self, _: usize) -> &mut View {
        self
    }
}
