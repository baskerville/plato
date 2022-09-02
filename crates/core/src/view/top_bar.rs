use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::gesture::GestureEvent;
use crate::input::DeviceEvent;
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, ViewId, Align};
use crate::view::icon::Icon;
use crate::view::clock::Clock;
use crate::view::battery::Battery;
use crate::view::label::Label;
use crate::geom::{Rectangle};
use crate::font::Fonts;
use crate::context::Context;

#[derive(Debug)]
pub struct TopBar {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
}

impl TopBar {
    pub fn new(rect: Rectangle, root_event: Event, title: String, context: &mut Context) -> TopBar {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();

        let side = rect.height() as i32;
        let icon_name = match root_event {
            Event::Back => "back",
            _ => "search",
        };

        let root_icon = Icon::new(icon_name,
                                  rect![rect.min, rect.min+side],
                                  root_event);
        children.push(Box::new(root_icon) as Box<dyn View>);

        let mut clock_rect = rect![rect.max - pt!(4*side, side),
                                   rect.max - pt!(3*side, 0)];
        let clock_label = Clock::new(&mut clock_rect, context);
        let title_rect = rect![rect.min.x + side, rect.min.y,
                               clock_rect.min.x, rect.max.y];
        let title_label = Label::new(title_rect, title, Align::Center)
                                .event(Some(Event::ToggleNear(ViewId::TitleMenu, title_rect)));
        children.push(Box::new(title_label) as Box<dyn View>);
        children.push(Box::new(clock_label) as Box<dyn View>);

        let capacity = context.battery.capacity().map_or(0.0, |v| v[0]);
        let status = context.battery.status().map_or(crate::battery::Status::Discharging, |v| v[0]);
        let battery_widget = Battery::new(rect![rect.max - pt!(3*side, side),
                                                rect.max - pt!(2*side, 0)],
                                          capacity,
                                          status);
        children.push(Box::new(battery_widget) as Box<dyn View>);

        let name = if context.settings.frontlight { "frontlight" } else { "frontlight-disabled" };
        let frontlight_icon = Icon::new(name,
                                        rect![rect.max - pt!(2*side, side),
                                              rect.max - pt!(side, 0)],
                                        Event::Show(ViewId::Frontlight));
        children.push(Box::new(frontlight_icon) as Box<dyn View>);

        let menu_rect = rect![rect.max-side, rect.max];
        let menu_icon = Icon::new("menu",
                                  menu_rect,
                                  Event::ToggleNear(ViewId::MainMenu, menu_rect));
        children.push(Box::new(menu_icon) as Box<dyn View>);

        TopBar {
            id,
            rect,
            children,
        }
    }

    pub fn update_root_icon(&mut self, name: &str, rq: &mut RenderQueue) {
        let icon = self.child_mut(0).downcast_mut::<Icon>().unwrap();
        if icon.name != name {
            icon.name = name.to_string();
            rq.add(RenderData::new(icon.id(), *icon.rect(), UpdateMode::Gui));
        }
    }

    pub fn update_title_label(&mut self, title: &str, rq: &mut RenderQueue) {
        let title_label = self.children[1].as_mut().downcast_mut::<Label>().unwrap();
        title_label.update(title, rq);
    }

    pub fn update_frontlight_icon(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        let name = if context.settings.frontlight { "frontlight" } else { "frontlight-disabled" };
        let icon = self.child_mut(4).downcast_mut::<Icon>().unwrap();
        icon.name = name.to_string();
        rq.add(RenderData::new(icon.id(), *icon.rect(), UpdateMode::Gui));
    }

    pub fn update_clock_label(&mut self, rq: &mut RenderQueue) {
        if let Some(clock_label) = self.children[2].downcast_mut::<Clock>() {
            clock_label.update(rq);
        }
    }

    pub fn update_battery_widget(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(battery_widget) = self.children[3].downcast_mut::<Battery>() {
            battery_widget.update(rq, context);
        }
    }

    pub fn reseed(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        self.update_frontlight_icon(rq, context);
        self.update_clock_label(rq);
        self.update_battery_widget(rq, context);
    }
}

impl View for TopBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, end, .. }) if self.rect.includes(start) && self.rect.includes(end) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let side = rect.height() as i32;
        self.children[0].resize(rect![rect.min, rect.min+side], hub, rq, context);
        let clock_width = self.children[2].rect().width() as i32;
        let clock_rect = rect![rect.max - pt!(3*side + clock_width, side),
                               rect.max - pt!(3*side, 0)];
        self.children[1].resize(rect![rect.min.x + side,
                                      rect.min.y,
                                      clock_rect.min.x,
                                      rect.max.y],
                                hub, rq, context);
        self.children[2].resize(clock_rect, hub, rq, context);
        self.children[3].resize(rect![rect.max - pt!(3*side, side),
                                      rect.max - pt!(2*side, 0)],
                                hub, rq, context);
        self.children[4].resize(rect![rect.max - pt!(2*side, side),
                                      rect.max - pt!(side, 0)],
                                hub, rq, context);
        self.children[5].resize(rect![rect.max-side, rect.max],
                                hub, rq, context);
        self.rect = rect;
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
