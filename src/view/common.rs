use std::env;
use view::{View, Event, Hub, ViewId, EntryId, EntryKind};
use framebuffer::UpdateMode;
use geom::{Point, Rectangle};
use view::menu::Menu;
use app::Context;

pub fn shift(view: &mut View, delta: &Point) {
    *view.rect_mut() += *delta;
    for child in view.children_mut().iter_mut() {
        shift(child.as_mut(), delta);
    }
}

pub fn locate<T: View>(view: &View) -> Option<usize> {
    for (index, child) in view.children().iter().enumerate() {
        if child.as_ref().is::<T>() {
            return Some(index);
        }
    }
    None
}

pub fn locate_by_id(view: &View, id: ViewId) -> Option<usize> {
    view.children().iter().position(|c| c.id().map_or(false, |i| i == id))
}

pub fn toggle_main_menu(view: &mut View, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
    let fonts = &mut context.fonts;

    if let Some(index) = locate_by_id(view, ViewId::MainMenu) {
        if let Some(true) = enable {
            return;
        }
        hub.send(Event::Expose(*view.child(index).rect())).unwrap();
        view.children_mut().remove(index);
    } else {
        if let Some(false) = enable {
            return;
        }
        let mut entries = vec![EntryKind::CheckBox("Invert Colors".to_string(),
                                                   EntryId::ToggleInverted,
                                                   context.inverted),
                               EntryKind::CheckBox("Make Bitonal".to_string(),
                                                   EntryId::ToggleMonochrome,
                                                   context.monochrome),
                               EntryKind::CheckBox("Enable WiFi".to_string(),
                                                   EntryId::ToggleWifi,
                                                   context.wifi),
                               EntryKind::Separator,
                               EntryKind::Command("Take Screenshot".to_string(),
                                                  EntryId::TakeScreenshot),
                               EntryKind::Separator];
        if env::var("ksmroot").is_ok() {
            entries.push(EntryKind::Command("Quit".to_string(), EntryId::Quit));
        } else {
            entries.extend_from_slice(&[EntryKind::Command("Suspend".to_string(),
                                                           EntryId::Suspend),
                                        EntryKind::Command("Power Off".to_string(),
                                                           EntryId::PowerOff),
                                        EntryKind::Command("Reboot".to_string(),
                                                           EntryId::Reboot),
                                        EntryKind::Command("Start Nickel".to_string(),
                                                           EntryId::StartNickel)]);
        }
        let main_menu = Menu::new(rect, ViewId::MainMenu, true, &entries, fonts);
        hub.send(Event::Render(*main_menu.rect(), UpdateMode::Gui)).unwrap();
        view.children_mut().push(Box::new(main_menu) as Box<View>);
    }
}
