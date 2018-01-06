use framebuffer::Framebuffer;
use metadata::Info;
use gesture::GestureEvent;
use view::{View, Event, Hub, Bus, ViewId, Align};
use view::icon::Icon;
use view::clock::Clock;
use view::battery::Battery;
use view::label::Label;
use geom::{Rectangle};
use color::WHITE;
use font::Fonts;
use app::Context;

#[derive(Debug)]
pub struct TopBar {
    rect: Rectangle,
    children: Vec<Box<View>>,
}

impl TopBar {
    pub fn new(rect: Rectangle, info: &Info, context: &mut Context) -> TopBar {
        let mut children = Vec::new();
        let fonts = &mut context.fonts;

        let side = rect.height() as i32;
        let root_icon = Icon::new("back",
                                  rect![rect.min, rect.min+side],
                                  Event::Back);
        children.push(Box::new(root_icon) as Box<View>);

        let mut clock_rect = rect![rect.max - pt!(4*side, side),
                                   rect.max - pt!(3*side, 0)];
        let clock_label = Clock::new(&mut clock_rect, fonts);
        children.push(Box::new(clock_label) as Box<View>);

        let title_label = Label::new(rect![rect.min.x + side, rect.min.y,
                                           clock_rect.min.x, rect.max.y],
                                     info.title(),
                                     Align::Center);
        children.push(Box::new(title_label) as Box<View>);

        let capacity = context.battery.capacity().unwrap_or(0.0);
        let status = context.battery.status().unwrap_or(::battery::Status::Discharging);
        let battery_widget = Battery::new(rect![rect.max - pt!(3*side, side),
                                                rect.max - pt!(2*side, 0)],
                                          capacity,
                                          status);
        children.push(Box::new(battery_widget) as Box<View>);

        let name = if context.settings.frontlight { "frontlight" } else { "frontlight-disabled" };
        let frontlight_icon = Icon::new(name,
                                        rect![rect.max - pt!(2*side, side),
                                              rect.max - pt!(side, 0)],
                                        Event::Show(ViewId::Frontlight));
        children.push(Box::new(frontlight_icon) as Box<View>);

        let menu_rect = rect![rect.max-side, rect.max];
        let menu_icon = Icon::new("menu",
                                  menu_rect,
                                  Event::ToggleNear(ViewId::MainMenu, menu_rect));
        children.push(Box::new(menu_icon) as Box<View>);

        TopBar {
            rect,
            children,
        }
    }
}

impl View for TopBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { ref start, .. }) if self.rect.includes(start) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {
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
