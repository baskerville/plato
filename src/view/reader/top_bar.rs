use framebuffer::{Framebuffer, UpdateMode};
use metadata::Info;
use gesture::GestureEvent;
use input::DeviceEvent;
use view::{View, Event, Hub, Bus, ViewId, Align};
use view::icon::Icon;
use view::clock::Clock;
use view::battery::Battery;
use view::label::Label;
use geom::{Rectangle};
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
        let title_label = Label::new(rect![rect.min.x + side, rect.min.y,
                                           clock_rect.min.x, rect.max.y],
                                     info.title(),
                                     Align::Center);
        children.push(Box::new(title_label) as Box<View>);
        children.push(Box::new(clock_label) as Box<View>);

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

    pub fn update_frontlight_icon(&mut self, hub: &Hub, context: &mut Context) {
        let name = if context.settings.frontlight { "frontlight" } else { "frontlight-disabled" };
        let icon = self.child_mut(4).downcast_mut::<Icon>().unwrap();
        icon.name = name.to_string();
        hub.send(Event::Render(*icon.rect(), UpdateMode::Gui)).unwrap();
    }
}

impl View for TopBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFinger(center)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, context: &mut Context) {
        let side = rect.height() as i32;
        self.children[0].resize(rect![rect.min, rect.min+side], hub, context);
        let clock_width = self.children[2].rect().width() as i32;
        let clock_rect = rect![rect.max - pt!(3*side + clock_width, side),
                               rect.max - pt!(3*side, 0)];
        self.children[1].resize(rect![rect.min.x + side,
                                      rect.min.y,
                                      clock_rect.min.x,
                                      rect.max.y],
                                hub, context);
        self.children[2].resize(clock_rect, hub, context);
        self.children[3].resize(rect![rect.max - pt!(3*side, side),
                                      rect.max - pt!(2*side, 0)],
                                hub, context);
        self.children[4].resize(rect![rect.max - pt!(2*side, side),
                                      rect.max - pt!(side, 0)],
                                hub, context);
        self.children[5].resize(rect![rect.max-side, rect.max],
                                hub, context);
        self.rect = rect;
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
