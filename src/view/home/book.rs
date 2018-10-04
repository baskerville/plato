use device::CURRENT_DEVICE;
use framebuffer::{Framebuffer, UpdateMode};
use view::{View, Event, Hub, Bus, THICKNESS_SMALL};
use font::{MD_TITLE, MD_AUTHOR, MD_YEAR, MD_KIND, MD_SIZE};
use color::{BLACK, WHITE, READING_PROGRESS};
use color::{TEXT_NORMAL, TEXT_INVERTED_HARD};
use gesture::GestureEvent;
use metadata::{Info, Status};
use settings::SecondColumn;
use unit::scale_by_dpi;
use document::HumanSize;
use font::{Fonts, font_from_style};
use geom::{Rectangle, CornerSpec, BorderSpec, halves};
use app::Context;

const PROGRESS_HEIGHT: f32 = 13.0;

pub struct Book {
    rect: Rectangle,
    children: Vec<Box<View>>,
    info: Info,
    index: usize,
    second_column: SecondColumn,
    active: bool,
}

impl Book {
    pub fn new(rect: Rectangle, info: Info, index: usize, second_column: SecondColumn) -> Book {
        Book {
            rect,
            children: vec![],
            info,
            index,
            second_column,
            active: false,
        }
    }
}

impl View for Book {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(ref center)) if self.rect.includes(*center) => {
                self.active = true;
                hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                hub.send(Event::Open(Box::new(self.info.clone()))).unwrap();
                true
            },
            Event::Gesture(GestureEvent::HoldFinger(ref center)) if self.rect.includes(*center) => {
                let pt = pt!(center.x, self.rect.center().y);
                bus.push_back(Event::ToggleBookMenu(Rectangle::from_point(pt), self.index));
                true
            },
            Event::Invalid(ref info) => {
                if self.info.file.path == info.file.path {
                    self.active = false;
                    hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                    true
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let scheme = if self.active {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        fb.draw_rectangle(&self.rect, scheme[0]);

        let title = self.info.title();
        let author = self.info.author();
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
            font.render(fb, scheme[1], &plan, pt);
            plan.width as i32
        };

        // Title
        {
            let font = font_from_style(fonts, &MD_TITLE, dpi);
            let mut plan = font.plan(&title, None, None);
            if plan.width > width as u32 {
                let available = width - author_width;
                if available > 3 * padding {
                    let (index, usable_width) = font.cut_point(&plan, width as u32);
                    let leftover = (plan.width - usable_width) as i32;
                    if leftover > 2 * padding {
                        let mut plan2 = plan.split_off(index, usable_width);
                        let max_width = available - padding;
                        font.crop_right(&mut plan2, max_width as u32);
                        let pt = pt!(self.rect.min.x + first_width - small_half_padding - plan2.width as i32,
                                     self.rect.max.y - baseline);
                        font.render(fb, scheme[1], &plan2, pt);
                    } else {
                        font.crop_right(&mut plan, width as u32);
                    }
                } else {
                    font.crop_right(&mut plan, width as u32);
                }
            }
            let pt = self.rect.min + pt!(padding, baseline + x_height);
            font.render(fb, scheme[1], &plan, pt);
        }

        // Year or Progress
        match self.second_column {
            SecondColumn::Year => {
                let font = font_from_style(fonts, &MD_YEAR, dpi);
                let plan = font.plan(year, None, None);
                let dx = (second_width - padding - plan.width as i32) / 2;
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
                        let color = if self.info.reader.is_none() { WHITE } else { READING_PROGRESS };
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
            let letter_spacing = scale_by_dpi(3.0, dpi) as u32;
            plan.space_out(letter_spacing);
            let pt = pt!(self.rect.max.x - padding - plan.width as i32,
                         self.rect.min.y + baseline + x_height);
            font.render(fb, scheme[1], &plan, pt);
        }

        // File size
        {
            let size = file_info.size.human_size();
            let font = font_from_style(fonts, &MD_SIZE, dpi);
            let plan = font.plan(&size, None, None);
            let pt = pt!(self.rect.max.x - padding - plan.width as i32,
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

    fn children(&self) -> &Vec<Box<View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<View>> {
        &mut self.children
    }
}
