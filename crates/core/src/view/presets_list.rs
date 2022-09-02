use crate::device::CURRENT_DEVICE;
use crate::geom::{Rectangle, Dir, CycleDir};
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::framebuffer::{Framebuffer, UpdateMode};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use super::preset::{Preset, PresetKind};
use crate::gesture::GestureEvent;
use crate::settings::LightPreset;
use crate::color::WHITE;
use crate::context::Context;

pub struct PresetsList {
    id: Id,
    rect: Rectangle,
    pages: Vec<Vec<Box<dyn View>>>,
    current_page: usize,
}

impl PresetsList {
    pub fn new(rect: Rectangle) -> PresetsList {
        PresetsList {
            id: ID_FEEDER.next(),
            rect,
            pages: Vec::new(),
            current_page: 0,
        }
    }

    pub fn update(&mut self, presets: &[LightPreset], rq: &mut RenderQueue, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let preset_height = 4 * x_height;
        let padding = font.em() as i32;
        let preset_width = font.plan(&presets[0].name(), None, None).width + padding;
        let max_per_line = (self.rect.width() as i32 + padding) / (preset_width + padding);

        self.pages.clear();
        let mut children = Vec::new();

        let presets_count = presets.len() as i32;
        let first_line_count = max_per_line.min(presets_count);
        let mut item_index = 0;
        let mut index = 0;

        let dx = (self.rect.width() as i32 - (first_line_count * preset_width +
                                              (first_line_count - 1) * padding)) / 2;

        while index < presets_count {
            let position = item_index % max_per_line;
            let x = self.rect.min.x + dx + position * (preset_width + padding);
            let preset_rect = rect![x, self.rect.max.y - preset_height,
                                    x + preset_width, self.rect.max.y];
            let kind = if (position == 0 && index > 0) || (position == max_per_line - 1 &&
                                                           index < presets_count - 1) {
                let dir = if position == 0 { CycleDir::Previous } else { CycleDir::Next };
                PresetKind::Page(dir)
            } else {
                let name = presets[index as usize].name();
                let kind = PresetKind::Normal(name, index as usize);
                index += 1;
                kind
            };

            let preset = Preset::new(preset_rect, kind);
            children.push(Box::new(preset) as Box<dyn View>);
            item_index += 1;

            if item_index % max_per_line == 0 || index == presets_count {
                self.pages.push(children);
                children = Vec::new();
            }
        }

        self.current_page = self.current_page.min(self.pages.len().saturating_sub(1));

        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    pub fn set_current_page(&mut self, dir: CycleDir) {
        match dir {
            CycleDir::Next if self.current_page < self.pages.len() - 1 => {
                self.current_page += 1;
            },
            CycleDir::Previous if self.current_page > 0 => {
                self.current_page -= 1;
            },
            _ => (),
        }
    }
}

impl View for PresetsList {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => {
                        self.set_current_page(CycleDir::Next);
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                        true
                    },
                    Dir::East => {
                        self.set_current_page(CycleDir::Previous);
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                        true
                    },
                    _ => false,
                }
            },
            Event::Page(dir) => {
                self.set_current_page(dir);
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        fb.draw_rectangle(&self.rect, WHITE);
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
        &self.pages[self.current_page]
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.pages[self.current_page]
    }

    fn id(&self) -> Id {
        self.id
    }
}
