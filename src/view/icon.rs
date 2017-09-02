use std::path::Path;
use std::sync::mpsc::Sender;
use fnv::FnvHashMap;
use device::CURRENT_DEVICE;
use framebuffer::{Framebuffer, Bitmap, UpdateMode};
use view::{View, Event, ChildEvent};
use gesture::GestureEvent;
use input::{FingerStatus, DeviceEvent};
use document::pdf::PdfOpener;
use unit::scale_by_dpi;
use font::Fonts;
use geom::{Rectangle, CornerSpec};
use color::{TEXT_NORMAL, TEXT_INVERTED};

const ICON_SCALE: f32 = 1.0 / 32.0;

lazy_static! {
    pub static ref ICONS_BITMAPS: FnvHashMap<&'static str, Bitmap> = {
        let mut m = FnvHashMap::default();
        let scale = scale_by_dpi(ICON_SCALE, CURRENT_DEVICE.dpi);
        let dir = Path::new("icons");
        for name in ["home", "search", "frontlight", "menu", "angle-left-small", "angle-right-small",
                     "delete-backward", "delete-forward", "move-backward", "move-forward", "close",
                     "check_mark", "bullet", "arrow-left", "arrow-right", "double_angle-left",
                     "double_angle-right", "angle-down"].iter().cloned() {
            let path = dir.join(&format!("{}.svg", name));
            let doc = PdfOpener::new().and_then(|o| o.open(path)).unwrap();
            let bitmap = doc.page(0).and_then(|p| p.render(scale)).unwrap();
            m.insert(name, bitmap);
        }
        m
    };
}

pub struct Icon {
    rect: Rectangle,
    corners: CornerSpec,
    event: ChildEvent,
    name: String,
    active: bool,
}

impl Icon {
    pub fn new(name: &str, rect: Rectangle, corners: CornerSpec, event: ChildEvent) -> Icon {
        Icon {
            rect,
            corners,
            name: name.to_string(),
            event,
            active: false,
        }
    }
}

impl View for Icon {
    fn handle_event(&mut self, evt: &Event, bus: &mut Vec<ChildEvent>) -> bool {
        match *evt {
            Event::GestureEvent(GestureEvent::Relay(de)) => {
                match de {
                    DeviceEvent::Finger { status, ref position, .. } => {
                        match status {
                            FingerStatus::Down if self.rect.includes(position) => {
                                self.active = true;
                                bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                false
                            },
                            FingerStatus::Up => {
                                self.active = false;
                                bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                false
                            },
                            FingerStatus::Motion => {
                                let active = self.rect.includes(position);
                                if active != self.active {
                                    self.active = active;
                                    bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                }
                                false
                            }
                            _ => false,
                        }
                    },
                    _ => false,
                }
            },
            Event::GestureEvent(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => {
                bus.push(self.event);
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, _: &mut Fonts) {
        let (foreground, background) = if self.active {
            (TEXT_INVERTED[1], TEXT_INVERTED[0])
        } else {
            (TEXT_NORMAL[1], TEXT_NORMAL[0])
        };
        let bitmap = ICONS_BITMAPS.get(&self.name[..]).unwrap();
        let dx = (self.rect.width() as i32 - bitmap.width) / 2;
        let dy = (self.rect.height() as i32 - bitmap.height) / 2;
        let pt = self.rect.min + pt!(dx, dy);
        fb.draw_rounded_rectangle(&self.rect, &self.corners, background);
        fb.draw_blended_bitmap(bitmap, &pt, foreground);
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn len(&self) -> usize {
        0
    }

    fn child(&self, _: usize) -> &View {
        self
    }

    fn child_mut(&mut self, _: usize) -> &mut View {
        self
    }
}
