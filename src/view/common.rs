use std::env;
use std::sync::mpsc;
use chrono::Local;
use framebuffer::UpdateMode;
use geom::{Point, Rectangle};
use super::{View, Event, Hub, ViewId, EntryId, EntryKind};
use super::menu::{Menu, MenuKind};
use super::notification::Notification;
use app::Context;

pub fn shift(view: &mut View, delta: Point) {
    *view.rect_mut() += delta;
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

pub fn overlapping_rectangle(view: &View) -> Rectangle {
    let mut rect = *view.rect();
    for child in view.children() {
        rect.absorb(&overlapping_rectangle(child.as_ref()));
    }
    rect
}

// Transfer the notifications from the view1 to the view2.
pub fn transfer_notifications(view1: &mut View, view2: &mut View, context: &mut Context) {
    for index in (0..view1.len()).rev() {
        if view1.child(index).is::<Notification>() {
            let mut child = view1.children_mut().remove(index);
            if view2.rect() != view1.rect() {
                let (tx, _rx) = mpsc::channel();
                child.resize(*view2.rect(), &tx, context);
            }
            view2.children_mut().push(child);
        }
    }
}

pub fn toggle_main_menu(view: &mut View, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
    if let Some(index) = locate_by_id(view, ViewId::MainMenu) {
        if let Some(true) = enable {
            return;
        }
        hub.send(Event::Expose(*view.child(index).rect(), UpdateMode::Gui)).unwrap();
        view.children_mut().remove(index);
    } else {
        if let Some(false) = enable {
            return;
        }
        let rotate = (0..4).map(|n| EntryKind::RadioButton((n as i16 * 90).to_string(),
                                                           EntryId::Rotate(n),
                                                           n == context.display.rotation))
                           .collect::<Vec<EntryKind>>();
        let mut entries = vec![EntryKind::CheckBox("Invert Colors".to_string(),
                                                   EntryId::ToggleInverted,
                                                   context.inverted),
                               EntryKind::CheckBox("Make Bitonal".to_string(),
                                                   EntryId::ToggleMonochrome,
                                                   context.monochrome),
                               EntryKind::CheckBox("Enable WiFi".to_string(),
                                                   EntryId::ToggleWifi,
                                                   context.settings.wifi),
                               EntryKind::Separator,
                               EntryKind::SubMenu("Rotate".to_string(), rotate),
                               EntryKind::Command("Take Screenshot".to_string(),
                                                  EntryId::TakeScreenshot),
                               EntryKind::Separator];
        if env::var("PLATO_STANDALONE").is_ok() {
            entries.extend_from_slice(&[EntryKind::Command("Start Nickel".to_string(),
                                                           EntryId::StartNickel),
                                        EntryKind::Command("Reboot".to_string(),
                                                           EntryId::Reboot)]);
        } else {
            entries.push(EntryKind::Command("Quit".to_string(), EntryId::Quit));
        }

        let main_menu = Menu::new(rect, ViewId::MainMenu, MenuKind::DropDown, entries, context);
        hub.send(Event::Render(*main_menu.rect(), UpdateMode::Gui)).unwrap();
        view.children_mut().push(Box::new(main_menu) as Box<View>);
    }
}

pub fn toggle_battery_menu(view: &mut View, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
    if let Some(index) = locate_by_id(view, ViewId::BatteryMenu) {
        if let Some(true) = enable {
            return;
        }
        hub.send(Event::Expose(*view.child(index).rect(), UpdateMode::Gui)).unwrap();
        view.children_mut().remove(index);
    } else {
        if let Some(false) = enable {
            return;
        }
        let text = match (context.battery.status(), context.battery.capacity()) {
            (Ok(status), Ok(capacity)) => format!("{:?} {}%", status, capacity),
            (Ok(status), Err(..)) => format!("{:?}", status),
            (Err(..), Ok(capacity)) => format!("{} %", capacity),
            _ => "Unknown".to_string(),
        };
        let entries = vec![EntryKind::Message(text)];
        let battery_menu = Menu::new(rect, ViewId::BatteryMenu, MenuKind::DropDown, entries, context);
        hub.send(Event::Render(*battery_menu.rect(), UpdateMode::Gui)).unwrap();
        view.children_mut().push(Box::new(battery_menu) as Box<View>);
    }
}

pub fn toggle_clock_menu(view: &mut View, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
    if let Some(index) = locate_by_id(view, ViewId::ClockMenu) {
        if let Some(true) = enable {
            return;
        }
        hub.send(Event::Expose(*view.child(index).rect(), UpdateMode::Gui)).unwrap();
        view.children_mut().remove(index);
    } else {
        if let Some(false) = enable {
            return;
        }
        let text = Local::now().format("%A, %B %-d, %Y").to_string();
        let entries = vec![EntryKind::Message(text)];
        let clock_menu = Menu::new(rect, ViewId::ClockMenu, MenuKind::DropDown, entries, context);
        hub.send(Event::Render(*clock_menu.rect(), UpdateMode::Gui)).unwrap();
        view.children_mut().push(Box::new(clock_menu) as Box<View>);
    }
}
