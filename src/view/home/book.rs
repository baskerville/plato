use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, THICKNESS_SMALL};
use crate::font::{MD_TITLE, MD_AUTHOR, MD_YEAR, MD_KIND, MD_SIZE};
use crate::color::{BLACK, WHITE, READING_PROGRESS};
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD};
use crate::gesture::GestureEvent;
use crate::metadata::{Info, Status};
use crate::settings::{FirstColumn, SecondColumn};
use crate::unit::scale_by_dpi;
use crate::document::HumanSize;
use crate::font::{Fonts, font_from_style};
use crate::geom::{Rectangle, CornerSpec, BorderSpec, halves};
use crate::app::Context;

const PROGRESS_HEIGHT: f32 = 13.0;

pub struct Book {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    info: Info,
    index: usize,
    first_column: FirstColumn,
    second_column: SecondColumn,
    active: bool,
}

impl Book {
    pub fn new(rect: Rectangle, info: Info, index: usize, first_column: FirstColumn, second_column: SecondColumn) -> Book {
        Book {
            id: ID_FEEDER.next(),
            rect,
            children: vec![],
            info,
            index,
            first_column,
            second_column,
            active: false,
        }
    }
}

impl View for Book {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                self.active = true;
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                hub.send(Event::Open(Box::new(self.info.clone()))).ok();
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => {
                let pt = pt!(center.x, self.rect.center().y);
                bus.push_back(Event::ToggleBookMenu(Rectangle::from_point(pt), self.index));
                true
            },
            Event::Invalid(ref info) => {
                if self.info.file.path == info.file.path {
                    self.active = false;
                    rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                    true
                } else {
                    false
                }
            },
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

        let (title, author) = if self.first_column == FirstColumn::TitleAndAuthor {
            (self.info.title(), self.info.author.as_str())
        } else {
            let filename = self.info.file.path.file_stem()
                               .map(|v| v.to_string_lossy().into_owned())
                               .unwrap_or_default();
            (filename, "")
        };

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
            let plan = font.plan(author, Some(width), None);
            let pt = pt!(self.rect.min.x + padding, self.rect.max.y - baseline);
            font.render(fb, scheme[1], &plan, pt);
            plan.width
        };

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
                        let pt = pt!(self.rect.min.x + first_width - small_half_padding - plan2.width,
                                     self.rect.max.y - baseline);
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

            let pt = self.rect.min + pt!(padding, dy);
            font.render(fb, scheme[1], &plan, pt);
        }

        // Year or Progress
        match self.second_column {
            SecondColumn::Year => {
                let font = font_from_style(fonts, &MD_YEAR, dpi);
                let plan = font.plan(year, None, None);
                let dx = (second_width - padding - plan.width) / 2;
                let dy = (self.rect.height() as i32 - font.x_heights.1 as i32) / 2;
                let pt = pt!(self.rect.min.x + first_width + big_half_padding + dx,
                             self.rect.max.y - dy);
                font.render(fb, scheme[1], &plan, pt);
            },
            SecondColumn::Progress => {
                let progress_height = scale_by_dpi(PROGRESS_HEIGHT, dpi) as i32;
                let thickness = scale_by_dpi(THICKNESS_SMALL, dpi) as u16;
                let (small_radius, big_radius) = halves(progress_height);
                let center = pt!(self.rect.min.x + first_width + second_width / 2,
                                 self.rect.min.y + self.rect.height() as i32 / 2);
                match self.info.status() {
                    Status::New | Status::Finished => {
                        let color = if self.info.reader.is_none() { WHITE } else { BLACK };
                        fb.draw_rounded_rectangle_with_border(&rect![center - pt!(small_radius, small_radius),
                                                                     center + pt!(big_radius, big_radius)],
                                                              &CornerSpec::Uniform(small_radius),
                                                              &BorderSpec { thickness, color: BLACK },
                                                              &color);
                    },
                    Status::Reading(progress) => {
                        let progress_width = 2 * (second_width - padding) / 3;
                        let (small_progress_width, big_progress_width) = halves(progress_width);
                        let x_offset = center.x - progress_width / 2 +
                                       (progress_width as f32 * progress.min(1.0)) as i32;
                        fb.draw_rounded_rectangle_with_border(&rect![center - pt!(small_progress_width, small_radius),
                                                                     center + pt!(big_progress_width, big_radius)],
                                                              &CornerSpec::Uniform(small_radius),
                                                              &BorderSpec { thickness, color: BLACK },
                                                              &|x, _| if x < x_offset { READING_PROGRESS } else { WHITE });
                    }
                }
            },
        }

        // File kind
        {
            let kind = file_info.kind.to_uppercase();
            let font = font_from_style(fonts, &MD_KIND, dpi);
            let mut plan = font.plan(&kind, None, None);
            let letter_spacing = scale_by_dpi(3.0, dpi) as i32;
            plan.space_out(letter_spacing);
            let pt = pt!(self.rect.max.x - padding - plan.width,
                         self.rect.min.y + baseline + x_height);
            font.render(fb, scheme[1], &plan, pt);
        }

        // File size
        {
            let size = file_info.size.human_size();
            let font = font_from_style(fonts, &MD_SIZE, dpi);
            let plan = font.plan(&size, None, None);
            let pt = pt!(self.rect.max.x - padding - plan.width,
                         self.rect.max.y - baseline);
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
