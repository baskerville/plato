use crate::{
    articles::Article,
    color::{SEPARATOR_NORMAL, TEXT_INVERTED_HARD, TEXT_NORMAL, WHITE},
    context::Context,
    device::CURRENT_DEVICE,
    font::{font_from_style, Fonts, MD_AUTHOR, MD_SIZE, MD_TITLE},
    framebuffer::{Framebuffer, UpdateMode},
    geom::{divide, halves, CycleDir, Dir, Rectangle},
    gesture::GestureEvent,
    unit::scale_by_dpi,
    view::{
        filler::Filler, Bus, Event, Hub, Id, RenderData, RenderQueue, View, BIG_BAR_HEIGHT,
        ID_FEEDER, THICKNESS_MEDIUM,
    },
};

pub struct Shelf {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
}

impl Shelf {
    pub fn new(rect: Rectangle) -> Shelf {
        Shelf {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
        }
    }

    pub fn max_lines(&self) -> usize {
        let dpi = CURRENT_DEVICE.dpi;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;

        ((self.rect.height() as i32 + thickness) / big_height) as usize
    }

    pub fn update(&mut self, metadata: &[Article], rq: &mut RenderQueue) {
        self.children.clear();
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let max_lines = self.max_lines();
        let title_heights = divide(self.rect.height() as i32, max_lines as i32);
        let mut y_pos = self.rect.min.y;

        for (index, article) in metadata.iter().enumerate() {
            let y_min = y_pos + if index > 0 { big_thickness } else { 0 };
            let y_max = y_pos + title_heights[index]
                - if index < max_lines - 1 {
                    small_thickness
                } else {
                    0
                };

            let title = Title::new(
                rect![self.rect.min.x, y_min, self.rect.max.x, y_max],
                article.clone(),
                index,
            );
            self.children.push(Box::new(title) as Box<dyn View>);

            if index < max_lines - 1 {
                let separator = Filler::new(
                    rect![self.rect.min.x, y_max, self.rect.max.x, y_max + thickness],
                    SEPARATOR_NORMAL,
                );
                self.children.push(Box::new(separator) as Box<dyn View>);
            }

            y_pos += title_heights[index];
        }

        if metadata.len() < max_lines {
            let y_start = y_pos + if metadata.is_empty() { 0 } else { thickness };
            let filler = Filler::new(
                rect![self.rect.min.x, y_start, self.rect.max.x, self.rect.max.y],
                WHITE,
            );
            self.children.push(Box::new(filler) as Box<dyn View>);
        }

        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }
}

impl View for Shelf {
    fn handle_event(
        &mut self,
        evt: &Event,
        _hub: &Hub,
        bus: &mut Bus,
        _rq: &mut RenderQueue,
        _context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => {
                        bus.push_back(Event::Page(CycleDir::Next));
                        true
                    }
                    Dir::East => {
                        bus.push_back(Event::Page(CycleDir::Previous));
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {}

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

struct Title {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    article: Article,
    index: usize,
    active: bool,
}

impl Title {
    pub fn new(rect: Rectangle, article: Article, index: usize) -> Title {
        Title {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            article,
            index,
            active: false,
        }
    }
}

impl View for Title {
    fn handle_event(
        &mut self,
        evt: &Event,
        hub: &Hub,
        bus: &mut Bus,
        rq: &mut RenderQueue,
        _context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                self.active = true;
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                hub.send(Event::Open(Box::new(self.article.info()))).ok();
                true
            }
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..))
                if self.rect.includes(center) =>
            {
                let pt = pt!(center.x, self.rect.center().y);
                bus.push_back(Event::ToggleBookMenu(Rectangle::from_point(pt), self.index));
                true
            }
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let scheme = if self.active {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        fb.draw_rectangle(&self.rect, scheme[0]);

        let (x_height, padding, baseline) = {
            let font = font_from_style(fonts, &MD_TITLE, dpi);
            let x_height = font.x_heights.0 as i32;
            (
                x_height,
                font.em() as i32,
                (self.rect.height() as i32 - 2 * x_height) / 3,
            )
        };

        let width = self.rect.width() as i32 - padding * 2;
        let start_x = self.rect.min.x as i32 + padding;

        // Title
        {
            let font = font_from_style(fonts, &MD_TITLE, dpi);
            let plan = font.plan(self.article.title.clone(), Some(width), None);
            let pt = pt!(start_x, self.rect.min.y + baseline + x_height);
            font.render(fb, scheme[1], &plan, pt);
        }

        // Reading time
        let reading_time_width = {
            let text = format!("{} min read", self.article.reading_time);
            let font = font_from_style(fonts, &MD_SIZE, dpi);
            let plan = font.plan(text, None, None);
            let pt = pt!(
                self.rect.max.x - padding - plan.width,
                self.rect.max.y - baseline
            );
            font.render(fb, scheme[1], &plan, pt);

            plan.width
        };

        // Domain and star
        {
            let font = font_from_style(fonts, &MD_AUTHOR, dpi);
            let mut text = self.article.domain.clone();
            if self.article.starred {
                // TODO: draw an actual star icon instead of an asterisk.
                text = "* ".to_owned() + &text;
            }
            let plan = font.plan(text, Some(width - reading_time_width - padding), None);
            let pt = pt!(start_x, self.rect.max.y - baseline);
            font.render(fb, scheme[1], &plan, pt);
        }
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
