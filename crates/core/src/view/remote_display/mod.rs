use crate::context::Context;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, Pixmap, UpdateMode};
use crate::geom::{Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::view::{Bus, Event, Hub, RenderData, RenderQueue, View};
use crate::view::{Id, ID_FEEDER};
use anyhow::Error;
use flate2::bufread::ZlibDecoder;
use image::codecs::pnm::PnmDecoder;
use image::ImageDecoder;
use rumqttc::tokio_rustls::rustls::ClientConfig;
use rumqttc::{
    AsyncClient, ConnAck, ConnectReturnCode, Incoming, MqttOptions, QoS, TlsConfiguration,
    Transport,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::sync::{mpsc as std_mpsc, Arc};
use std::thread::spawn;
use std::time::Duration;
use tokio::sync::mpsc as tokio_mpsc;

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
    UpdateSize,
}

#[tokio::main(flavor = "current_thread")]
async fn display_connection(
    event_tx: std_mpsc::Sender<Event>,
    mut socket_rx: tokio_mpsc::Receiver<SocketEvent>,
    address: String,
    topic: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mqo = MqttOptions::parse_url(address)?;
    mqo.set_keep_alive(Duration::from_secs(30));
    mqo.set_max_packet_size(1_048_576, 1_048_576);
    let root_store = rumqttc::tokio_rustls::rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect(),
    };
    let cc = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let tlsc = TlsConfiguration::Rustls(Arc::new(cc));
    match mqo.transport() {
        Transport::Tls(..) => {
            mqo.set_transport(Transport::Tls(tlsc));
        }
        Transport::Wss(..) => {
            mqo.set_transport(Transport::Wss(tlsc));
        }
        _ => {}
    }
    let (client, mut eventloop) = AsyncClient::new(mqo, 10);
    let sub_topic = topic.clone() + "/device";
    let pub_topic = topic.clone() + "/browser";
    client.subscribe(sub_topic, QoS::ExactlyOnce).await?;
    event_tx.send(Event::Notify("Connected".to_string()))?;
    loop {
        tokio::select! {
            Some(socket_event) = socket_rx.recv() => {
                match socket_event {
                    SocketEvent::Finished => {
                        client.disconnect().await?;
                        break;
                    }
                    SocketEvent::SendJSON(value) => {
                        client.publish(pub_topic.clone(), QoS::ExactlyOnce, false, value.to_string()).await?;
                    }
                }
            }
            event = eventloop.poll() => {
                match event {
                    Ok(rumqttc::Event::Incoming(Incoming::Publish(p))) => {
                        let payload = p.payload;

                        if !payload.starts_with(&[123, 34]) {
                            event_tx.send(Event::UpdateRemoteView(Box::new(payload.to_vec())))?;
                            continue;
                        }
                        let message = serde_json::from_slice::<ServerMessage>(&payload)?;
                        event_tx.send(match message {
                            ServerMessage::Notify(message) => {
                                Event::Notify(message)
                            }
                            ServerMessage::RefreshDisplay => {
                                Event::Update(UpdateMode::Full)
                            }
                            ServerMessage::UpdateSize => {
                                Event::SendRemoteViewSize
                            }
                        })?;
                    }
                    Ok(rumqttc::Event::Incoming(
                        Incoming::ConnAck(ConnAck { session_present: _, code: ConnectReturnCode::Success }))
                    ) => {
                        event_tx.send(Event::SendRemoteViewSize)?;
                    }
                    Ok(..) => {}
                    Err(e) => {
                        println!("{}", e);
                        event_tx.send(Event::Notify(e.to_string()))?;
                    }
                }
            }
        }
    }
    event_tx.send(Event::Notify("Disconnected".to_string()))?;

    Ok(())
}

pub struct RemoteDisplay {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pixmap: Pixmap,
    message_tx: tokio_mpsc::Sender<SocketEvent>,
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

        let address = context.settings.remote_display.address.clone();
        let topic = context.settings.remote_display.topic.clone();

        let (message_tx, message_rx) = tokio_mpsc::channel(16);
        let event_tx = hub.clone();

        spawn(
            move || match display_connection(event_tx.clone(), message_rx, address, topic) {
                Ok(..) => {}
                Err(e) => {
                    event_tx.send(Event::Back).ok();
                    event_tx.send(Event::Notify(e.to_string())).ok();
                }
            },
        );

        RemoteDisplay {
            id,
            rect,
            children,
            pixmap: Pixmap::new(rect.width(), rect.height()),
            message_tx,
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
        self.message_tx
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
                self.message_tx.try_send(SocketEvent::Finished).ok();
                bus.push_back(Event::Back);
                true
            }
            Event::Gesture(ge) => {
                self.message_tx
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
                self.message_tx
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
            Event::SendRemoteViewSize => {
                self.message_tx
                    .try_send(SocketEvent::SendJSON(json!({
                        "type": "size",
                        "value": {
                            "width": self.rect.width(),
                            "height": self.rect.height(),
                        }
                    })))
                    .ok();
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

        self.rect = rect;
        hub.send(Event::SendRemoteViewSize).unwrap();
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
