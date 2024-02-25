use crate::context::Context;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, Pixmap, UpdateMode};
use crate::geom::{Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::view::{Bus, Event, Hub, RenderData, RenderQueue, View};
use crate::view::{Id, ID_FEEDER};
use anyhow::Error;
use base64::{engine::general_purpose, Engine as _};
use flate2::bufread::ZlibDecoder;
use futures_util::{SinkExt, StreamExt};
use image::codecs::pnm::PnmDecoder;
use image::ImageDecoder;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::sync::mpsc as std_mpsc;
use std::thread::spawn;
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[derive(Debug, Clone)]
enum SocketEvent {
    Finished,
    SendJSON(Value),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
enum ServerMessage {
    Notify(String),
    RefreshDisplay,
}

#[tokio::main(flavor = "current_thread")]
async fn display_connection(
    event_tx: std_mpsc::Sender<Event>,
    mut socket_rx: tokio_mpsc::Receiver<SocketEvent>,
    url: Url,
) -> Result<(), Box<dyn std::error::Error>> {
    let username = url.username();
    let password = url.password().unwrap_or("");

    let mut request = url.clone().into_client_request()?;
    match username {
        "" => {}
        username => {
            request.headers_mut().append(
                "Authorization",
                format!(
                    "Basic {}",
                    general_purpose::STANDARD
                        .encode(format!("{}:{}", username, password).as_bytes())
                )
                .parse()?,
            );
        }
    }

    let (mut socket, _) = connect_async(request).await?;
    event_tx.send(Event::Notify("Connected".to_string()))?;
    loop {
        tokio::select! {
            Some(event) = socket_rx.recv() => {
                match event {
                    SocketEvent::Finished => { break; }
                    SocketEvent::SendJSON(val) => {
                        socket.send(Message::Text(val.to_string())).await?;
                    }
                }
            }
            Some(msg) = socket.next() => {
                let text = match msg {
                    Ok(Message::Text(text)) => text,
                    Ok(Message::Binary(bin)) => {
                        event_tx.send(Event::UpdateRemoteView(Box::new(bin)))?;
                        continue;
                    },
                    Ok(Message::Close(_)) => { break; }
                    Ok(_) => { continue; }
                    Err(e) => {
                        event_tx.send(Event::Notify(e.to_string()))?;
                        break;
                    }
                };

                let server_msg = match serde_json::from_str::<ServerMessage>(&text) {
                    Ok(sm) => sm,
                    Err(e) => {
                        println!("{}", e);
                        println!("contents: {}", text);
                        event_tx.send(Event::Notify("Invalid message from server".to_string()))?;
                        continue;
                    }
                };

                match server_msg {
                    ServerMessage::Notify(msg) => {
                        event_tx.send(Event::Notify(msg))?;
                    }
                    ServerMessage::RefreshDisplay => {
                        event_tx.send(Event::Update(UpdateMode::Full))?;
                    }
                }
            }
        }
    }
    event_tx.send(Event::Notify("Disconnected".to_string()))?;
    socket.close(None).await?;

    Ok(())
}

pub struct RemoteDisplay {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pixmap: Pixmap,
    socket_tx: tokio_mpsc::Sender<SocketEvent>,
}

impl RemoteDisplay {
    pub fn new(
        rect: Rectangle,
        hub: &Hub,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) -> RemoteDisplay {
        let id = ID_FEEDER.next();
        let children = Vec::new();
        rq.add(RenderData::new(id, rect, UpdateMode::Full));

        let tx = hub.clone();
        let my_tx = hub.clone();

        let address = context.settings.remote_display.address.clone();
        let (socket_tx, socket_rx) = tokio_mpsc::channel(10);
        spawn(move || {
            let url = match Url::parse(&address) {
                Ok(url) => url,
                Err(e) => {
                    my_tx.send(Event::Back).ok();
                    tx.send(Event::Notify(e.to_string())).ok();
                    return;
                }
            };
            match display_connection(tx, socket_rx, url) {
                Ok(_) => {}
                Err(e) => {
                    my_tx.send(Event::Back).ok();
                    my_tx.send(Event::Notify(e.to_string())).ok();
                }
            }
        });
        socket_tx
            .try_send(SocketEvent::SendJSON(json!({
                "type": "size",
                "value": {
                    "width": rect.width(),
                    "height": rect.height(),
                }
            })))
            .ok();

        RemoteDisplay {
            id,
            rect,
            children,
            socket_tx,
            pixmap: Pixmap::new(rect.width(), rect.height()),
        }
    }

    fn update_remote_view(&mut self, deflated_data: Box<Vec<u8>>) -> Result<(), Error> {
        let mut inflated = Vec::new();
        ZlibDecoder::new(deflated_data.as_slice()).read_to_end(&mut inflated)?;
        let dec = PnmDecoder::new(inflated.as_slice())?;
        let (width, height) = dec.dimensions();
        let mut pixmap = Pixmap::new(width, height);
        dec.read_image(&mut pixmap.data_mut())?;
        self.pixmap = pixmap;
        self.socket_tx
            .try_send(SocketEvent::SendJSON(json!({
                "type": "displayUpdated",
            })))
            .ok();
        Ok(())
    }
}

impl View for RemoteDisplay {
    fn handle_event(
        &mut self,
        evt: &Event,
        hub: &Hub,
        bus: &mut Bus,
        rq: &mut RenderQueue,
        _context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Arrow {
                dir: Dir::South,
                start: _start,
                end: _end,
            }) => {
                self.socket_tx.try_send(SocketEvent::Finished).ok();
                bus.push_back(Event::Back);
                true
            }
            Event::Gesture(ge) => {
                self.socket_tx
                    .try_send(SocketEvent::SendJSON(serde_json::to_value(ge).unwrap()))
                    .ok();
                true
            }
            Event::Device(DeviceEvent::Button { code, status, .. }) => {
                let button = match code {
                    ButtonCode::Forward => "forward",
                    ButtonCode::Backward => "backward",
                    _ => return false,
                };
                let status = match status {
                    ButtonStatus::Pressed => "pressed",
                    ButtonStatus::Released => "released",
                    ButtonStatus::Repeated => "repeated",
                };
                self.socket_tx
                    .try_send(SocketEvent::SendJSON(json!({
                        "type": "button",
                        "value": {
                            "button": button,
                            "status": status,
                        }
                    })))
                    .ok();
                true
            }
            Event::UpdateRemoteView(ref pbm_data) => {
                let data = pbm_data.clone();
                match self.update_remote_view(data) {
                    Ok(..) => {}
                    Err(e) => {
                        println!("{}", e);
                        hub.send(Event::Notify(e.to_string())).unwrap();
                    }
                }
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                true
            }
            Event::Update(UpdateMode::Full) => {
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
                true
            }
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        let max_pixel_x = (rect.max.x - 1) as u32;
        let max_pixel_y = (rect.max.y - 1) as u32;
        let addr = (max_pixel_y * self.pixmap.width + max_pixel_x) as usize;
        let out_of_bounds = addr >= self.pixmap.data.len();
        if out_of_bounds {
            // Blank to prevent a panic when the pixmap is not the expected size.
            let pixmap = Pixmap::new(fb.width(), fb.height());
            fb.draw_framed_pixmap(&pixmap, &rect, rect.min);
            return;
        }
        fb.draw_framed_pixmap(&self.pixmap, &rect, rect.min);
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect).unwrap_or(self.rect)
    }

    fn is_background(&self) -> bool {
        true
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        // Floating windows.
        for i in 0..self.children.len() {
            self.children[i].resize(rect, hub, rq, context);
        }

        self.socket_tx
            .try_send(SocketEvent::SendJSON(json!({
                "type": "size",
                "value": {
                    "width": rect.width(),
                    "height": rect.height(),
                }
            })))
            .ok();

        self.rect = rect;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
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