use framebuffer::Framebuffer;
use font::{Fonts, font_from_style, NORMAL_STYLE};
use geom::{Rectangle, Point, CornerSpec, BorderSpec, halves, big_half};
use view::{View, Event, Hub, Bus, ViewId, Align};
use view::{THICKNESS_LARGE, BORDER_RADIUS_MEDIUM};
use view::label::Label;
use view::input_field::InputField;
use unit::scale_by_dpi;
use color::{BLACK, WHITE};
use device::CURRENT_DEVICE;
use app::Context;

const LABEL_TEXT: &str = "Go to page";

pub struct GoToPage {
    rect: Rectangle,
    children: Vec<Box<View>>,
}

impl GoToPage {
    pub fn new(anchor: &Point, pages_count: usize, fonts: &mut Fonts) -> GoToPage {
        let mut children = Vec::new();
        let font = font_from_style(fonts, &NORMAL_STYLE, CURRENT_DEVICE.dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;

        let label_width = font.plan(LABEL_TEXT, None, None).width as i32;
        let input_field_width = font.plan(&format!("{}", pages_count), None, None).width as i32;
        let width = 5 * padding + label_width + input_field_width;
        let (small_half_width, big_half_width) = halves(width);
        let big_half_padding = big_half(padding);

        let x_min = anchor.x - small_half_width;
        let x_max = anchor.x + big_half_width;
        let y_min = anchor.y - 4 * x_height;
        let y_max = anchor.y + 4 * x_height;

        let label = Label::new(rect![x_min + big_half_padding,
                                     y_min + x_height,
                                     x_min + big_half_padding + padding + label_width,
                                     y_max - x_height],
                               LABEL_TEXT.to_string(),
                               Align::Center);
        children.push(Box::new(label) as Box<View>);

        let input_field = InputField::new(rect![x_max - 3 * padding - input_field_width,
                                                y_min + 2 * x_height,
                                                x_max - padding,
                                                y_max - 2 * x_height],
                                          ViewId::GoToPageInput,
                                          true,
                                          None);
        children.push(Box::new(input_field) as Box<View>);

        let rect = rect![x_min, y_min,
                         x_max, y_max];
                                          
        GoToPage {
            rect,
            children,
        }
    }
}

impl View for GoToPage {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Submit(ViewId::GoToPageInput, ref text) => {
                if let Ok(index) = text.parse::<usize>() {
                    bus.push_back(Event::GoTo(index.saturating_sub(1)));
                    bus.push_back(Event::Close(ViewId::GoToPage));
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;
        fb.draw_rounded_rectangle_with_border(&self.rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness,
                                                            color: BLACK },
                                              &WHITE);
                                              // &::color::GRAY13);
    }

    fn is_background(&self) -> bool {
        true
    }

    fn id(&self) -> Option<ViewId> {
        Some(ViewId::GoToPage)
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
