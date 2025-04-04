use crate::context::Context;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, Pixmap, UpdateMode};
use crate::geom::{Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::view::{Bus, Event, Hub, RenderData, RenderQueue, View};
use crate::view::{Id, ID_FEEDER};
use bytes::Buf;
use chacha20poly1305::KeyInit;
use chacha20poly1305::{
    aead::generic_array::GenericArray, aead::OsRng, AeadCore, AeadInPlace, ChaCha20Poly1305,
};
use coset::cbor::{cbor, de::from_reader, ser::into_writer, Value};
use coset::{CborSerializable, CoseEncrypt0, CoseEncrypt0Builder, HeaderBuilder};
use rumqttc::tokio_rustls::rustls::ClientConfig;
use rumqttc::{
    AsyncClient, ConnAck, ConnectReturnCode, Incoming, MqttOptions, QoS, TlsConfiguration,
    Transport,
};
use serde::Deserialize;
use std::io::{Cursor, Read};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};
use std::thread::spawn;
use std::time::Duration;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::time::sleep;

#[derive(Debug, Clone)]
enum SocketEvent {
    Finished,
    SendMessage(Value),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
enum ServerMessage {
    Notify(String),
    RefreshDisplay,
    UpdateSize,
    UpdateDisplay(Vec<u8>),
    PatchDisplay(Vec<u8>),
}

fn handle_server_message(
    message: ServerMessage,
    event_tx: &std::sync::mpsc::Sender<Event>,
    prev_frame: &mut Option<Vec<u8>>,
    shared_pixmap: &Arc<Mutex<UpdatedPixmap>>,
) -> anyhow::Result<()> {
    let event = match message {
        ServerMessage::Notify(message) => Event::Notify(message),
        ServerMessage::RefreshDisplay => Event::Update(UpdateMode::Full),
        ServerMessage::UpdateSize => Event::SendRemoteViewSize,
        ServerMessage::UpdateDisplay(data) => {
            let qoi = zstd::decode_all(data.as_slice())?;
            *prev_frame = Some(qoi.clone());
            let rgb = image::load_from_memory(&qoi)?.to_rgb8();
            let width = rgb.width() as u32;
            let height = rgb.height() as u32;
            let data = rgb.into_raw();
            if let Ok(mut updated) = shared_pixmap.lock() {
                updated.pixmap = Pixmap {
                    width,
                    height,
                    samples: 3,
                    data,
                };
                updated.full = true;
                updated.notified = false;
            }
            Event::Update(UpdateMode::Gui)
        }
        ServerMessage::PatchDisplay(data) => {
            let decompressed = zstd::decode_all(data.as_slice())?;
            let patched_buf = {
                let prev = prev_frame
                    .as_ref()
                    .ok_or(anyhow::anyhow!("Served interframe with no previous frame"))?;
                let mut slice = decompressed.as_slice();
                let mut patched = bipatch::Reader::new(&mut slice, Cursor::new(prev))?;
                let mut buf = Vec::new();
                patched.read_to_end(&mut buf)?;
                buf
            };
            *prev_frame = Some(patched_buf.clone());
            let img = image::load_from_memory(&patched_buf)?;
            let rgb = img.to_rgb8();
            let width = rgb.width() as u32;
            let height = rgb.height() as u32;
            let data = rgb.into_raw();
            if let Ok(mut updated) = shared_pixmap.lock() {
                updated.pixmap = Pixmap {
                    width,
                    height,
                    samples: 3,
                    data,
                };
                updated.full = false;
                updated.notified = false;
            }
            Event::Update(UpdateMode::Gui)
        }
    };
    event_tx.send(event)?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn display_connection(
    event_tx: std_mpsc::Sender<Event>,
    mut socket_rx: tokio_mpsc::Receiver<SocketEvent>,
    shared_pixmap: Arc<Mutex<UpdatedPixmap>>,
    address: String,
    topic: String,
    hex_key: String,
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

    let header = HeaderBuilder::new()
        .algorithm(coset::iana::Algorithm::ChaCha20Poly1305)
        .build();

    let key = hex::decode(hex_key)?;
    let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(key.as_slice()));

    let mut prev_frame: Option<Vec<u8>> = None;

    loop {
        tokio::select! {
            Some(socket_event) = socket_rx.recv() => {
                match socket_event {
                    SocketEvent::Finished => {
                        client.disconnect().await?;
                        break;
                    }
                    SocketEvent::SendMessage(value) => {
                        let mut writer = Vec::new();
                        into_writer(&value, &mut writer)?;
                        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
                        let payload = CoseEncrypt0Builder::new()
                            .protected(header.clone())
                            .unprotected(HeaderBuilder::new().iv(nonce.into_iter().collect()).build())
                            .try_create_ciphertext(writer.as_slice(), &[], |pt, aad| {
                                let mut out = pt.to_vec();
                                cipher.encrypt_in_place(&nonce, aad, &mut out)
                                    .map_err(|e| anyhow::anyhow!("Encryption error: {}", e.to_string()))
                                    .map(|_| out)
                            })?
                            .build()
                            .to_vec()
                            .map_err(|e| anyhow::anyhow!("Encryption error: {}", e.to_string()))?;
                        client.publish(pub_topic.clone(), QoS::AtMostOnce, false, payload).await?;
                    }
                }
            }
            event = eventloop.poll() => {
                match event {
                    Ok(rumqttc::Event::Incoming(Incoming::Publish(p))) => {
                        let ct_obj = CoseEncrypt0::from_slice(p.payload.chunk())
                            .map_err(|e| anyhow::anyhow!("Encrypted message format error: {}", e.to_string()))?;
                        let pt = ct_obj.decrypt(&[], |pt, aad| {
                                let mut out = pt.to_vec();
                                cipher.decrypt_in_place(
                                    GenericArray::from_slice(ct_obj.unprotected.iv.as_slice()),
                                    aad, &mut out
                                )
                                    .map_err(|e| anyhow::anyhow!("Decryption error: {}", e.to_string()))
                                    .map(|_| out)
                            })?;
                        let message: ServerMessage = from_reader(pt.as_slice())?;
                        if let Err(e) = handle_server_message(message, &event_tx, &mut prev_frame, &shared_pixmap) {
                            event_tx.send(Event::Notify(format!("Error handling message: {}", e))).ok();
                        }
                    }
                    Ok(rumqttc::Event::Incoming(
                        Incoming::ConnAck(ConnAck { session_present: _, code: ConnectReturnCode::Success }))
                    ) => {
                        event_tx.send(Event::Notify("Connected".to_string()))?;
                        client.subscribe(sub_topic.clone(), QoS::AtMostOnce).await?;
                        event_tx.send(Event::SendRemoteViewSize)?;
                    }
                    Ok(..) => {}
                    Err(e) => {
                        println!("{}", e);
                        event_tx.send(Event::Notify(e.to_string()))?;
                        sleep(Duration::from_millis(2000)).await;
                    }
                }
            }
        }
    }
    event_tx.send(Event::Notify("Disconnected".to_string()))?;

    Ok(())
}

struct UpdatedPixmap {
    pixmap: Pixmap,
    full: bool,
    notified: bool,
}

pub struct RemoteDisplay {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pixmap: Arc<Mutex<UpdatedPixmap>>,
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
        let key = context.settings.remote_display.key.clone();

        let (message_tx, message_rx) = tokio_mpsc::channel(16);
        let event_tx = hub.clone();

        let pixmap = Arc::new(Mutex::new(UpdatedPixmap {
            pixmap: Pixmap::new(rect.width(), rect.height(), 1),
            full: false,
            notified: true,
        }));
        let shared_pixmap = pixmap.clone();

        spawn(move || {
            match display_connection(
                event_tx.clone(),
                message_rx,
                shared_pixmap,
                address,
                topic,
                key,
            ) {
                Ok(..) => {}
                Err(e) => {
                    event_tx.send(Event::Back).ok();
                    event_tx.send(Event::Notify(e.to_string())).ok();
                }
            }
        });

        RemoteDisplay {
            id,
            rect,
            children,
            pixmap,
            message_tx,
        }
    }
}

impl View for RemoteDisplay {
    fn handle_event(
        &mut self,
        evt: &Event,
        _hub: &Hub,
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
                    .try_send(SocketEvent::SendMessage(cbor!(ge).unwrap()))
                    .ok();
                match ge {
                    GestureEvent::HoldFingerShort(pt, _) | GestureEvent::Tap(pt) => {
                        if let Ok(mut locked_pixmap) = self.pixmap.lock() {
                            locked_pixmap.pixmap.draw_crosshair(pt, 5.0);
                        }
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                        true
                    }
                    _ => true,
                }
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
                    .try_send(SocketEvent::SendMessage(
                        cbor!({
                            "type" => "button",
                            "value" => {
                                "button" => button,
                                "status" => status,
                            }
                        })
                        .unwrap(),
                    ))
                    .ok();
                true
            }
            Event::SendRemoteViewSize => {
                self.message_tx
                    .try_send(SocketEvent::SendMessage(
                        cbor!({
                            "type" => "size",
                            "value" => {
                                "width" => self.rect.width(),
                                "height" => self.rect.height(),
                            }
                        })
                        .unwrap(),
                    ))
                    .ok();
                true
            }
            Event::Update(UpdateMode::Full) => {
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
                true
            }
            Event::Update(UpdateMode::Gui) => {
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                true
            }
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        if let Ok(mut locked_pixmap) = self.pixmap.lock() {
            let pixmap = &locked_pixmap.pixmap;
            if fb.width() != pixmap.width || fb.height() != pixmap.height || pixmap.data.is_empty()
            {
                // Create a blank pixmap with correct dimensions to prevent panics
                let blank = Pixmap::new(fb.width(), fb.height(), 1);
                fb.draw_framed_pixmap(&blank, &rect, rect.min);
                return;
            }
            fb.draw_framed_pixmap(pixmap, &rect, rect.min);
            if !locked_pixmap.notified {
                cbor!({ "type" => "displayUpdated", "full" => locked_pixmap.full })
                    .map_err(|e| anyhow::anyhow!("Encoding displayUpdated response: {}", e))
                    .and_then(|msg| {
                        self.message_tx
                            .try_send(SocketEvent::SendMessage(msg))
                            .map_err(|e| anyhow::anyhow!("Sending displayUpdated response: {}", e))
                    })
                    .inspect_err(|e| println!("{}", e))
                    .ok();
                locked_pixmap.notified = true;
            }
        } else {
            // If mutex is poisoned or cannot be locked, draw blank
            let pixmap = Pixmap::new(fb.width(), fb.height(), 1);
            fb.draw_framed_pixmap(&pixmap, &rect, rect.min);
        }
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
