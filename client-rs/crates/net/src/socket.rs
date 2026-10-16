//! The world server connection. Messages sent before the connection opens are queued and go
//! out once it does, as `SocketServer.as` does.

use bsk_proto::{ClientMessage, FrameDecoder, ServerMessage};

/// Where to connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
    /// The gateway's WebSocket base, e.g. `wss://example.org/ws` (always set in a browser). The
    /// connection goes to `<gateway>/<host>:<port>`; without one, TCP to `host:port`.
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

    /// How long a WebSocket read waits before sending what's queued: the most a message waits.
    const WS_POLL: Duration = Duration::from_millis(5);

    pub struct GameSocket {
        events: Receiver<SocketEvent>,
        out: Sender<Vec<u8>>,
        /// The TCP socket underneath, to shut it down from here.
        stream: Arc<Mutex<Option<TcpStream>>>,
    }

    impl GameSocket {
        pub fn connect(target: &Target) -> Self {
            let (event_tx, events) = channel();
            let (out, out_rx) = channel::<Vec<u8>>();
            let stream_slot = Arc::new(Mutex::new(None));
            let slot = stream_slot.clone();
            let target = target.clone();
            std::thread::Builder::new()
                .name("game-socket".into())
                .spawn(move || {
                    let reason = match &target.gateway {
                        Some(gateway) => {
                            let url = format!("{}/{}:{}", gateway.trim_end_matches('/'), target.host, target.port);
                            run_ws(&url, &slot, &event_tx, out_rx).err().unwrap_or_else(|| "closed".into())
                        }
                        None => run_tcp(&target.host, target.port, &slot, &event_tx, out_rx),
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

    /// Straight to the world server; returns why it ended.
    fn run_tcp(
        host: &str,
        port: u16,
        slot: &Mutex<Option<TcpStream>>,
        event_tx: &Sender<SocketEvent>,
        out_rx: Receiver<Vec<u8>>,
    ) -> String {
        let stream = match open(host, port) {
            Ok(s) => s,
            Err(e) => return format!("can't connect to {host}:{port}: {e}"),
        };
        let (Ok(mut reader), Ok(mut writer)) = (stream.try_clone(), stream.try_clone()) else {
            return "socket clone failed".into();
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
        loop {
            match reader.read(&mut buf) {
                Ok(0) => return "the server closed the connection".to_owned(),
                Ok(n) => {
                    if let Err(e) = decode_into(&mut decoder, &buf[..n], |ev| {
                        let _ = event_tx.send(ev);
                    }) {
                        return e;
                    }
                }
                Err(e) => return e.to_string(),
            }
        }
    }

    /// Through the gateway's WebSocket bridge (wss: encrypted), as the browser client goes. One
    /// thread reads with a short timeout and sends what's queued in between, since the
    /// connection can't be split between a reader and a writer.
    fn run_ws(
        url: &str,
        slot: &Mutex<Option<TcpStream>>,
        event_tx: &Sender<SocketEvent>,
        out_rx: Receiver<Vec<u8>>,
    ) -> Result<(), String> {
        use tungstenite::{http::Uri, Connector, Message};
        let uri: Uri = url.parse().map_err(|e| format!("{url}: {e}"))?;
        let host = uri.host().ok_or(format!("{url}: no host"))?;
        let port = uri.port_u16().unwrap_or(if uri.scheme_str() == Some("wss") { 443 } else { 80 });
        let tcp = open(host, port).map_err(|e| format!("can't connect to {host}:{port}: {e}"))?;
        tcp.set_read_timeout(Some(Duration::from_secs(15))).map_err(|e| e.to_string())?;
        // A second handle, to shut the connection down from `close`. (Read timeouts go on the
        // stream itself: on Windows a clone is a handle of its own, with its own timeout.)
        *slot.lock().unwrap() = Some(tcp.try_clone().map_err(|e| e.to_string())?);
        let (mut ws, _) = tungstenite::client_tls_with_config(url, tcp, None, Some(Connector::Rustls(crate::tls::config())))
            .map_err(|e| format!("{url}: {e}"))?;
        let stream = match ws.get_mut() {
            tungstenite::stream::MaybeTlsStream::Plain(s) => s,
            tungstenite::stream::MaybeTlsStream::Rustls(s) => s.get_mut(),
            _ => return Err("unexpected stream".into()),
        };
        stream.set_read_timeout(Some(WS_POLL)).map_err(|e| e.to_string())?;
        let _ = event_tx.send(SocketEvent::Connected);
        let mut decoder = FrameDecoder::default();
        loop {
            loop {
                match out_rx.try_recv() {
                    Ok(frame) => ws.send(Message::Binary(frame)).map_err(|e| e.to_string())?,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let _ = ws.close(None);
                        return Ok(());
                    }
                }
            }
            match ws.read() {
                Ok(Message::Binary(bytes)) => decode_into(&mut decoder, &bytes, |ev| {
                    let _ = event_tx.send(ev);
                })?,
                Ok(Message::Close(_)) => return Err("the server closed the connection".into()),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => return Err(e.to_string()),
            }
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
