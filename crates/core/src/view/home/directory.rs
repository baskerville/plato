use std::path::{PathBuf, Path};
use crate::device::CURRENT_DEVICE;
use crate::gesture::GestureEvent;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::color::{WHITE, BLACK, TEXT_BUMP_SMALL};
use crate::geom::{Rectangle, CornerSpec, BorderSpec};
use crate::framebuffer::Framebuffer;
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, Align};
use crate::view::{THICKNESS_SMALL, BORDER_RADIUS_SMALL};
use crate::unit::scale_by_dpi;
use crate::context::Context;

pub struct Directory {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pub path: PathBuf,
    selected: bool,
    align: Align,
    max_width: Option<i32>,
}

impl Directory {
    pub fn new(rect: Rectangle, path: PathBuf, selected: bool, align: Align, max_width: Option<i32>) -> Directory {
        Directory {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            path,
            selected,
            align,
            max_width,
        }
    }

    pub fn update_selected(&mut self, current_directory: &Path) -> bool {
        let selected = current_directory.starts_with(&self.path);
        self.selected = selected;
        selected
    }
}

impl View for Directory {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                bus.push_back(Event::ToggleSelectDirectory(self.path.clone()));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        fb.draw_rectangle(&self.rect, TEXT_BUMP_SMALL[0]);
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let text = self.path.file_name().unwrap().to_string_lossy();
        let plan = font.plan(text, self.max_width, None);

        let dx = self.align.offset(plan.width, self.rect.width() as i32);
        let dy = (self.rect.height() as i32 - x_height) / 2;

        if self.selected {
            let padding = font.em() as i32 / 2 - scale_by_dpi(3.0, dpi) as i32;
            let small_x_height = font.x_heights.0 as i32;
            let bg_width = plan.width + 2 * padding;
            let bg_height = 3 * small_x_height;
            let x_offset = dx - padding;
            let y_offset = dy + x_height - 2 * small_x_height;
            let pt = self.rect.min + pt!(x_offset, y_offset);
            let bg_rect = rect![pt, pt + pt!(bg_width, bg_height)];
            let border_radius = scale_by_dpi(BORDER_RADIUS_SMALL, dpi) as i32;
            let border_thickness = scale_by_dpi(THICKNESS_SMALL, dpi) as u16;
            fb.draw_rounded_rectangle_with_border(&bg_rect,
                                                  &CornerSpec::Uniform(border_radius),
                                                  &BorderSpec { thickness: border_thickness,
                                                                color: BLACK },
                                                  &WHITE);
        }

        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
        font.render(fb, TEXT_BUMP_SMALL[1], &plan, pt);
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
