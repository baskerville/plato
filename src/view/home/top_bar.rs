use framebuffer::{Framebuffer, UpdateMode};
use view::{View, Event, Hub, Bus, ViewId};
use view::icon::Icon;
use view::clock::Clock;
use view::home::sort_label::SortLabel;
use view::battery::Battery;
use metadata::SortMethod;
use app::Context;
use font::Fonts;
use geom::{Rectangle};

#[derive(Debug)]
pub struct TopBar {
    rect: Rectangle,
    children: Vec<Box<View>>,
}

impl TopBar {
    pub fn new(rect: Rectangle, sort_method: SortMethod, context: &mut Context) -> TopBar {
        let mut children = Vec::new();
        let fonts = &mut context.fonts;

        let side = rect.height() as i32;
        let root_icon = Icon::new("search",
                                  rect![rect.min, rect.min+side],
                                  Event::Toggle(ViewId::SearchBar));
        children.push(Box::new(root_icon) as Box<View>);

        let mut clock_rect = rect![rect.max - pt!(4*side, side),
                                   rect.max - pt!(3*side, 0)];
        let clock_label = Clock::new(&mut clock_rect, fonts);
        let sort_label = SortLabel::new(rect![pt!(rect.min.x + side,
                                                  rect.min.y),
                                              pt!(clock_rect.min.x,
                                                  rect.max.y)],
                                        sort_method.label());
        children.push(Box::new(sort_label) as Box<View>);
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

    // TODO: only update if needed
    pub fn update_icons(&mut self, search_visible: bool, hub: &Hub, context: &mut Context) {
        self.update_root_icon(search_visible, hub);
        self.update_frontlight_icon(hub, context);
    }

    pub fn update_root_icon(&mut self, search_visible: bool, hub: &Hub) {
        let icon = self.child_mut(0).downcast_mut::<Icon>().unwrap();
        let name = if search_visible { "home" } else { "search" };
        icon.name = name.to_string();
        hub.send(Event::Render(*icon.rect(), UpdateMode::Gui)).unwrap();
    }

    pub fn update_frontlight_icon(&mut self, hub: &Hub, context: &mut Context) {
        let name = if context.settings.frontlight { "frontlight" } else { "frontlight-disabled" };
        let icon = self.child_mut(4).downcast_mut::<Icon>().unwrap();
        icon.name = name.to_string();
        hub.send(Event::Render(*icon.rect(), UpdateMode::Gui)).unwrap();
    }

    pub fn update_sort_label(&mut self, sort_method: SortMethod, hub: &Hub) {
        let sort_label = self.children[1].as_mut().downcast_mut::<SortLabel>().unwrap();
        sort_label.update(sort_method.label(), hub);
    }
}

impl View for TopBar {
    fn handle_event(&mut self, _evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        false
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
