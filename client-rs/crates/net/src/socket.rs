//! The world server connection. Messages sent before the connection opens are queued and go
//! out once it does, as `SocketServer.as` does.

use bsk_proto::{ClientMessage, FrameDecoder, ServerMessage};

/// Where to connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
    /// Browsers only: the gateway's WebSocket base, e.g. `ws://example.org:8095/ws`. The
    /// connection goes to `<gateway>/<host>:<port>`.
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SocketEvent {
    Connected,
    Message(Box<ServerMessage>),
    /// The connection ended (or never opened); the text says why.
    Closed(String),
}

/// Split received bytes into messages; `Err` ends the connection.
fn decode_into(decoder: &mut FrameDecoder, bytes: &[u8], mut emit: impl FnMut(SocketEvent)) -> Result<(), String> {
    decoder.push(bytes);
    while let Some((id, body)) = decoder.next_frame().map_err(|e| e.to_string())? {
        match ServerMessage::decode(id, body) {
            Ok(msg) => emit(SocketEvent::Message(Box::new(msg))),
            Err(e) => return Err(format!("message {id}: {e}")),
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream, ToSocketAddrs};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    pub struct GameSocket {
        events: Receiver<SocketEvent>,
        out: Sender<Vec<u8>>,
        stream: Arc<Mutex<Option<TcpStream>>>,
    }

    impl GameSocket {
        pub fn connect(target: &Target) -> Self {
            let (event_tx, events) = channel();
            let (out, out_rx) = channel::<Vec<u8>>();
            let stream_slot = Arc::new(Mutex::new(None));
            let slot = stream_slot.clone();
            let (host, port) = (target.host.clone(), target.port);
            std::thread::Builder::new()
                .name("game-socket".into())
                .spawn(move || {
                    let stream = match open(&host, port) {
                        Ok(s) => s,
                        Err(e) => {
                            let _ = event_tx.send(SocketEvent::Closed(format!("can't connect to {host}:{port}: {e}")));
                            return;
                        }
                    };
                    let (Ok(mut reader), Ok(mut writer)) = (stream.try_clone(), stream.try_clone()) else {
                        let _ = event_tx.send(SocketEvent::Closed("socket clone failed".into()));
                        return;
                    };
                    *slot.lock().unwrap() = Some(stream);
                    let _ = event_tx.send(SocketEvent::Connected);
                    std::thread::spawn(move || {
                        for frame in out_rx {
                            if writer.write_all(&frame).is_err() {
                                break;
                            }
                        }
                    });
                    let mut decoder = FrameDecoder::default();
                    let mut buf = vec![0u8; 64 * 1024];
                    let reason = loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break "the server closed the connection".to_owned(),
                            Ok(n) => {
                                if let Err(e) = decode_into(&mut decoder, &buf[..n], |ev| {
                                    let _ = event_tx.send(ev);
                                }) {
                                    break e;
                                }
                            }
                            Err(e) => break e.to_string(),
                        }
                    };
                    let _ = event_tx.send(SocketEvent::Closed(reason));
                })
                .expect("spawn socket thread");
            Self { events, out, stream: stream_slot }
        }

        pub fn send(&mut self, msg: &ClientMessage) {
            let _ = self.out.send(msg.to_frame());
        }

        pub fn poll(&mut self) -> Option<SocketEvent> {
            self.events.try_recv().ok()
        }

        pub fn close(&mut self) {
            if let Some(s) = self.stream.lock().unwrap().take() {
                let _ = s.shutdown(Shutdown::Both);
            }
        }
    }

    impl Drop for GameSocket {
        fn drop(&mut self) {
            self.close();
        }
    }

    fn open(host: &str, port: u16) -> std::io::Result<TcpStream> {
        let mut last = None;
        for addr in (host, port).to_socket_addrs()? {
            match TcpStream::connect_timeout(&addr, Duration::from_secs(10)) {
                Ok(s) => {
                    s.set_nodelay(true)?;
                    return Ok(s);
                }
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| std::io::Error::other("no address")))
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::{BinaryType, CloseEvent, MessageEvent, WebSocket};

    #[derive(Default)]
    struct Shared {
        events: VecDeque<SocketEvent>,
        unsent: Vec<Vec<u8>>,
        decoder: FrameDecoder,
        closed: bool,
    }

    pub struct GameSocket {
        ws: Option<WebSocket>,
        shared: Rc<RefCell<Shared>>,
        _handlers: Vec<Box<dyn std::any::Any>>,
    }

    impl GameSocket {
        pub fn connect(target: &Target) -> Self {
            let shared = Rc::new(RefCell::new(Shared::default()));
            let Some(gateway) = &target.gateway else {
                shared.borrow_mut().events.push_back(SocketEvent::Closed("no gateway configured".into()));
                return Self { ws: None, shared, _handlers: vec![] };
            };
            let url = format!("{}/{}:{}", gateway.trim_end_matches('/'), target.host, target.port);
            let ws = match WebSocket::new(&url) {
                Ok(ws) => ws,
                Err(e) => {
                    shared.borrow_mut().events.push_back(SocketEvent::Closed(format!("{url}: {e:?}")));
                    return Self { ws: None, shared, _handlers: vec![] };
                }
            };
            ws.set_binary_type(BinaryType::Arraybuffer);

            let on_open = {
                let shared = shared.clone();
                let ws2 = ws.clone();
                Closure::<dyn FnMut()>::new(move || {
                    let mut s = shared.borrow_mut();
                    for frame in s.unsent.drain(..) {
                        let _ = ws2.send_with_u8_array(&frame);
                    }
                    s.events.push_back(SocketEvent::Connected);
                })
            };
            let on_message = {
                let shared = shared.clone();
                let ws2 = ws.clone();
                Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                    let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() else { return };
                    let bytes = js_sys::Uint8Array::new(&buf).to_vec();
                    let mut s = shared.borrow_mut();
                    let s = &mut *s;
                    let events = &mut s.events;
                    if let Err(err) = decode_into(&mut s.decoder, &bytes, |ev| events.push_back(ev)) {
                        events.push_back(SocketEvent::Closed(err));
                        s.closed = true;
                        let _ = ws2.close();
                    }
                })
            };
            let on_close = {
                let shared = shared.clone();
                Closure::<dyn FnMut(CloseEvent)>::new(move |e: CloseEvent| {
                    let mut s = shared.borrow_mut();
                    if !s.closed {
                        s.closed = true;
                        let why = if e.reason().is_empty() { format!("closed ({})", e.code()) } else { e.reason() };
                        s.events.push_back(SocketEvent::Closed(why));
                    }
                })
            };
            ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
            ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
            ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
            Self { ws: Some(ws), shared, _handlers: vec![Box::new(on_open), Box::new(on_message), Box::new(on_close)] }
        }

        pub fn send(&mut self, msg: &ClientMessage) {
            let frame = msg.to_frame();
            match &self.ws {
                Some(ws) if ws.ready_state() == WebSocket::OPEN => {
                    let _ = ws.send_with_u8_array(&frame);
                }
                Some(_) => self.shared.borrow_mut().unsent.push(frame),
                None => {}
            }
        }

        pub fn poll(&mut self) -> Option<SocketEvent> {
            self.shared.borrow_mut().events.pop_front()
        }

        pub fn close(&mut self) {
            if let Some(ws) = self.ws.take() {
                self.shared.borrow_mut().closed = true;
                ws.set_onopen(None);
                ws.set_onmessage(None);
                ws.set_onclose(None);
                let _ = ws.close();
            }
        }
    }

    impl Drop for GameSocket {
        fn drop(&mut self) {
            self.close();
        }
    }
}

pub use imp::GameSocket;
