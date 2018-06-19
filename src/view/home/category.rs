use device::CURRENT_DEVICE;
use gesture::GestureEvent;
use font::{Fonts, font_from_style, category_font_size, NORMAL_STYLE};
use color::{WHITE, BLACK, TEXT_BUMP_SMALL};
use geom::{Rectangle, CornerSpec, BorderSpec, Dir};
use framebuffer::Framebuffer;
use view::{View, Event, Hub, Bus, Align};
use view::{THICKNESS_SMALL, BORDER_RADIUS_SMALL};
use symbolic_path::SymbolicPath;
use unit::scale_by_dpi;
use app::Context;

pub struct Category {
    rect: Rectangle,
    children: Vec<Box<View>>,
    text: String,
    status: Status,
    align: Align,
    max_width: Option<u32>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Status {
    Normal,
    Selected,
    Negated,
}

impl Category {
    pub fn new(rect: Rectangle, text: String, status: Status, align: Align, max_width: Option<u32>) -> Category {
        Category {
            rect,
            children: vec![],
            text,
            status,
            align,
            max_width,
        }
    }
}

impl View for Category {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(ref center)) if self.rect.includes(*center) => {
                bus.push_back(Event::ToggleSelectCategory(self.text.clone()));
                true
            },
            Event::Gesture(GestureEvent::Swipe { dir: Dir::North, ref start, .. }) if self.rect.includes(*start) => {
                bus.push_back(Event::ToggleNegateCategory(self.text.clone()));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        fb.draw_rectangle(&self.rect, TEXT_BUMP_SMALL[0]);
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        font.set_size(category_font_size(self.text.depth()), dpi);
        let plan = font.plan(self.text.last_component(), self.max_width, None);

        let dx = self.align.offset(plan.width as i32, self.rect.width() as i32);
        let dy = (self.rect.height() as i32 - x_height) / 2;

        if self.status == Status::Selected {
            let padding = font.em() as i32 / 2 - scale_by_dpi(3.0, dpi) as i32;
            let small_x_height = font.x_heights.0 as i32;
            let bg_width = plan.width as i32 + 2 * padding;
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
        let color_index = if self.status == Status::Negated { 2 } else { 1 };
        font.render(fb, TEXT_BUMP_SMALL[color_index], &plan, pt);
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
