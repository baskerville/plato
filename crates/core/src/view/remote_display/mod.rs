use crate::context::Context;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, Pixmap, UpdateMode};
use crate::geom::{Dir, Rectangle};
use crate::gesture::GestureEvent;
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::view::{Bus, Event, Hub, RenderData, RenderQueue, View};
use crate::view::{Id, ID_FEEDER};
use anyhow::Error;
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
use std::sync::{mpsc as std_mpsc, Arc};
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

#[tokio::main(flavor = "current_thread")]
async fn display_connection(
    event_tx: std_mpsc::Sender<Event>,
    mut socket_rx: tokio_mpsc::Receiver<SocketEvent>,
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
                        let message = from_reader(pt.as_slice())?;
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
                            ServerMessage::UpdateDisplay(data) => {
                                Event::UpdateRemoteView(Box::new(data), true)
                            }
                            ServerMessage::PatchDisplay(data) => {
                                Event::UpdateRemoteView(Box::new(data), false)
                            }
                        })?;
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

pub struct RemoteDisplay {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pixmap: Pixmap,
    message_tx: tokio_mpsc::Sender<SocketEvent>,
    prev_frame: Option<Vec<u8>>,
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

        spawn(move || {
            match display_connection(event_tx.clone(), message_rx, address, topic, key) {
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
            pixmap: Pixmap::new(rect.width(), rect.height(), 1),
            message_tx,
            prev_frame: None,
        }
    }

    fn update_remote_view(&mut self, zst_data: &Box<Vec<u8>>, keyframe: bool) -> Result<(), Error> {
        let decompressed = zstd::stream::decode_all(zst_data.as_slice())?;
        let qoi = if keyframe {
            decompressed
        } else {
            let prev_frame = self
                .prev_frame
                .as_mut()
                .ok_or(anyhow::anyhow!("Served interframe with no previous frame"))?;

            let mut slice = decompressed.as_slice();
            let mut patched = bipatch::Reader::new(&mut slice, Cursor::new(prev_frame))?;
            let mut buf = Vec::new();
            patched.read_to_end(&mut buf)?;
            buf
        };

        self.prev_frame = Some(qoi.clone());
        let img = image::load_from_memory(&qoi)?;
        let rgb = img.to_rgb8();
        let mut pixmap = Pixmap::new(rgb.width() as u32, rgb.height() as u32, 3);
        pixmap.data.copy_from_slice(rgb.as_raw());
        self.pixmap = pixmap;
        self.message_tx.try_send(SocketEvent::SendMessage(cbor!({
            "type" => "displayUpdated",
            "full" => keyframe
        })?))?;

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
                    .try_send(SocketEvent::SendMessage(cbor!(ge).unwrap()))
                    .ok();
                match ge {
                    GestureEvent::HoldFingerShort(pt, _) | GestureEvent::Tap(pt) => {
                        self.pixmap.draw_crosshair(pt, 5.0);
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
            Event::UpdateRemoteView(ref zst_data, keyframe) => {
                match self.update_remote_view(zst_data, keyframe) {
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
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        if fb.width() != self.pixmap.width
            || fb.height() != self.pixmap.height
            || self.pixmap.data.is_empty()
        {
            // Create a blank pixmap with correct dimensions to prevent panics
            let pixmap = Pixmap::new(fb.width(), fb.height(), 1);
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
