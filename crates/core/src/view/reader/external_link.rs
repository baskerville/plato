use crate::color::{BLACK, WHITE};
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::{font_from_style, Fonts, NORMAL_STYLE};
use crate::framebuffer::Framebuffer;
use crate::geom::{BorderSpec, CornerSpec, Rectangle};
use crate::gesture::GestureEvent;
use crate::unit::scale_by_dpi;
use crate::view::button::Button;
use crate::view::icon::Icon;
use crate::view::label::Label;
use crate::view::{Align, Bus, Event, Hub, Id, RenderQueue, View, ViewId, ID_FEEDER};
use crate::view::{BORDER_RADIUS_MEDIUM, SMALL_BAR_HEIGHT, THICKNESS_LARGE};

const LABEL_QUEUE: &str = "Queue";
const LABEL_ADD_ARTICLE: &str = "Add article";

pub struct ExternalLink {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
}

impl ExternalLink {
    pub fn new(context: &mut Context, link: String) -> ExternalLink {
        let id = ID_FEEDER.next();
        let fonts = &mut context.fonts;
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = context.display.dims;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;
        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;

        let (x_height, padding) = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            (font.x_heights.0 as i32, font.em() as i32)
        };

        let window_width = width as i32 - 2 * padding;
        let window_height = small_height * 4 + 2 * padding;

        let dx = (width as i32 - window_width) / 2;
        let dy = (height as i32 - window_height) / 4;

        let rect = rect![dx, dy, dx + window_width, dy + window_height];

        let close_icon = Icon::new(
            "close",
            rect![
                rect.max.x - small_height,
                rect.min.y + thickness,
                rect.max.x - thickness,
                rect.min.y + small_height
            ],
            Event::Close(ViewId::ExternalLink),
        )
        .corners(Some(CornerSpec::Detailed {
            north_west: 0,
            north_east: border_radius - thickness,
            south_east: 0,
            south_west: 0,
        }));
        children.push(Box::new(close_icon) as Box<dyn View>);

        let label = Label::new(
            rect![
                rect.min.x + small_height,
                rect.min.y + thickness,
                rect.max.x - small_height,
                rect.min.y + small_height
            ],
            "External link".to_string(),
            Align::Center,
        );
        children.push(Box::new(label) as Box<dyn View>);

        // TODO: wrap the URL if needed.
        let link_label = Label::new(
            rect![
                rect.min.x + small_height,
                rect.min.y + thickness + small_height,
                rect.max.x - small_height,
                rect.min.y + 2 * small_height
            ],
            link.clone(),
            Align::Left(0),
        );
        children.push(Box::new(link_label) as Box<dyn View>);

        let max_button_label_width = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            [LABEL_QUEUE, LABEL_ADD_ARTICLE]
                .iter()
                .map(|t| font.plan(t, None, None).width)
                .max()
                .unwrap() as i32
        };

        let button_y = rect.min.y + small_height * 3;
        let button_height = 4 * x_height;

        let button_queue = Button::new(
            rect![
                rect.min.x + 3 * padding,
                button_y + small_height - button_height,
                rect.min.x + 5 * padding + max_button_label_width,
                button_y + small_height
            ],
            Event::QueueLink(link.clone()),
            LABEL_QUEUE.to_string(),
        )
        .disabled(context.settings.external_urls_queue.is_none());
        children.push(Box::new(button_queue) as Box<dyn View>);

        let button_add_article = Button::new(
            rect![
                rect.max.x - 5 * padding - max_button_label_width,
                button_y + small_height - button_height,
                rect.max.x - 3 * padding,
                button_y + small_height
            ],
            Event::AddArticleLink(link),
            LABEL_ADD_ARTICLE.to_string(),
        )
        .disabled(context.settings.article_auth.api == "");
        children.push(Box::new(button_add_article) as Box<dyn View>);

        ExternalLink { id, rect, children }
    }
}

impl View for ExternalLink {
    fn handle_event(
        &mut self,
        evt: &Event,
        _hub: &Hub,
        bus: &mut Bus,
        _rq: &mut RenderQueue,
        _context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if !self.rect.includes(center) => {
                bus.push_back(Event::Close(ViewId::ExternalLink));
                true
            }
            Event::Gesture(..) => true,
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;

        fb.draw_rounded_rectangle_with_border(
            &self.rect,
            &CornerSpec::Uniform(border_radius),
            &BorderSpec {
                thickness: border_thickness,
                color: BLACK,
            },
            &WHITE,
        );
    }

    fn is_background(&self) -> bool {
        true
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
