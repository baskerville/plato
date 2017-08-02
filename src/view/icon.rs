use std::path::Path;
use std::sync::mpsc::Sender;
use device::CURRENT_DEVICE;
use framebuffer::{Framebuffer, Bitmap};
use view::{View, Event, ChildEvent};
use gesture::GestureEvent;
use input::{FingerStatus, DeviceEvent};
use document::pdf::{PdfOpener, PdfDocument};
use unit::scale_by_dpi;
use framebuffer::UpdateMode;
use font::Fonts;
use geom::{Rectangle, CornerSpec};
use color::{TEXT_NORMAL, TEXT_INVERTED};

const ICON_SCALE: f32 = 1.0 / 32.0;

pub struct Icon {
    rect: Rectangle,
    corners: CornerSpec,
    bitmap: Bitmap,
    event: ChildEvent,
    active: bool,
}

impl Icon {
    pub fn new<P: AsRef<Path>>(path: P, rect: Rectangle, corners: CornerSpec, event: ChildEvent) -> Icon {
        let scale = scale_by_dpi(ICON_SCALE, CURRENT_DEVICE.dpi);
        let doc = PdfOpener::new().and_then(|o| o.open(path)).unwrap();
        let bitmap = doc.page(0).and_then(|p| p.render(scale)).unwrap();
        Icon {
            rect,
            corners,
            bitmap,
            event,
            active: false,
        }
    }
}

impl View for Icon {
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
    fn handle_event(&mut self, evt: &Event, bus: &Sender<ChildEvent>) -> bool {
        match *evt {
            Event::GestureEvent(GestureEvent::Relay(de)) => {
                match de {
                    DeviceEvent::Finger { status, ref position, .. } => {
                        match status {
                            FingerStatus::Down if self.rect.includes(position) => {
                                self.active = true;
                                bus.send(ChildEvent::Render(self.rect, UpdateMode::Gui)).unwrap();
                                false
                            },
                            FingerStatus::Up => {
                                self.active = false;
                                bus.send(ChildEvent::Render(self.rect, UpdateMode::Gui)).unwrap();
                                false
                            },
                            FingerStatus::Motion => {
                                let active = self.rect.includes(position);
                                if active != self.active {
                                    self.active = active;
                                    bus.send(ChildEvent::Render(self.rect, UpdateMode::Gui)).unwrap();
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
                bus.send(self.event).unwrap();
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
        let dx = (self.rect.width() as i32 - self.bitmap.width) / 2;
        let dy = (self.rect.height() as i32 - self.bitmap.height) / 2;
        let pt = self.rect.min + pt!(dx, dy);
        fb.draw_rounded_rectangle(&self.rect, &self.corners, background);
        fb.draw_blended_bitmap(&self.bitmap, &pt, foreground);
    }
}
