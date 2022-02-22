use std::path::Path;
use fxhash::FxHashMap;
use lazy_static::lazy_static;
use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, Pixmap, UpdateMode};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, ViewId, Align};
use crate::gesture::GestureEvent;
use crate::input::{DeviceEvent, FingerStatus};
use crate::document::pdf::PdfOpener;
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD};
use crate::unit::scale_by_dpi_raw;
use crate::geom::{Rectangle, CornerSpec};
use crate::font::Fonts;
use crate::app::Context;

const ICON_SCALE: f32 = 1.0 / 32.0;

lazy_static! {
    pub static ref ICONS_PIXMAPS: FxHashMap<&'static str, Pixmap> = {
        let mut m = FxHashMap::default();
        let scale = scale_by_dpi_raw(ICON_SCALE, CURRENT_DEVICE.dpi);
        let dir = Path::new("icons");
        for name in ["home", "search", "back", "frontlight", "frontlight-disabled", "menu",
                     "angle-left", "angle-right", "angle-left-small", "angle-right-small",
                     "return", "shift", "combine", "alternate", "delete-backward", "delete-forward",
                     "move-backward", "move-backward-short", "move-forward", "move-forward-short",
                     "close",  "check_mark-small", "check_mark", "check_mark-large", "bullet",
                     "arrow-left", "arrow-right", "angle-down", "angle-up", "crop", "toc", "font_family",
                     "font_size", "line_height", "align-justify", "align-left", "align-right",
                     "align-center", "margin", "plug", "cover", "enclosed_menu", "contrast", "gray"].iter().cloned() {
            let path = dir.join(&format!("{}.svg", name));
            let doc = PdfOpener::new().and_then(|o| o.open(path)).unwrap();
            let pixmap = doc.page(0).and_then(|p| p.pixmap(scale)).unwrap();
            m.insert(name, pixmap);
        }
        m
    };
}

pub struct Icon {
    id: Id,
    pub rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pub name: String,
    background: u8,
    align: Align,
    corners: Option<CornerSpec>,
    event: Event,
    pub active: bool,
}

impl Icon {
    pub fn new(name: &str, rect: Rectangle, event: Event) -> Icon {
        Icon {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            name: name.to_string(),
            background: TEXT_NORMAL[0],
            align: Align::Center,
            corners: None,
            event,
            active: false,
        }
    }

    pub fn background(mut self, background: u8) -> Icon {
        self.background = background;
        self
    }

    pub fn align(mut self, align: Align) -> Icon {
        self.align = align;
        self
    }

    pub fn corners(mut self, corners: Option<CornerSpec>) -> Icon {
        self.corners = corners;
        self
    }
}

impl View for Icon {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::Finger { status, position, .. }) => {
                match status {
                    FingerStatus::Down if self.rect.includes(position) => {
                        self.active = true;
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Fast));
                        true
                    },
                    FingerStatus::Up if self.active => {
                        self.active = false;
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                        true
                    },
                    _ => false,
                }
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                bus.push_back(self.event.clone());
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => {
                match self.event {
                    Event::Page(dir) => bus.push_back(Event::Chapter(dir)),
                    Event::Show(ViewId::Frontlight) => {
                        hub.send(Event::ToggleFrontlight).ok();
                    },
                    Event::Show(ViewId::MarginCropper) => {
                        bus.push_back(Event::ToggleNear(ViewId::MarginCropperMenu, self.rect));
                    },
                    Event::History(dir, false) => {
                        bus.push_back(Event::History(dir, true));
                    },
                    _ => (),
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        let scheme = if self.active {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        let pixmap = ICONS_PIXMAPS.get(&self.name[..]).unwrap();
        let dx = self.align.offset(pixmap.width as i32, self.rect.width() as i32);
        let dy = (self.rect.height() as i32 - pixmap.height as i32) / 2;
        let pt = self.rect.min + pt!(dx, dy);

        let background = if self.active {
            scheme[0]
        } else {
            self.background
        };

        if let Some(ref cs) = self.corners {
            fb.draw_rounded_rectangle(&self.rect, cs, background);
        } else {
            fb.draw_rectangle(&self.rect, background);
        }

        fb.draw_blended_pixmap(pixmap, pt, scheme[1]);
    }

    fn resize(&mut self, rect: Rectangle, _hub: &Hub, _rq: &mut RenderQueue, _context: &mut Context) {
        if let Event::ToggleNear(_, ref mut event_rect) = self.event {
            *event_rect = rect;
        }
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
