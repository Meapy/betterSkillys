//! The game connection over the gateway's WebSocket bridge, as the browser client makes it:
//! binary WebSocket messages carry the TCP byte stream, in any chunking.

use bsk_net::SocketEvent;
use bsk_proto::{ClientMessage, FrameDecoder, ServerMessage};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::Message;

pub struct WsSocket {
    events: Receiver<SocketEvent>,
    out: Sender<Option<Vec<u8>>>,
}

impl WsSocket {
    pub fn connect(url: &str) -> Self {
        let (event_tx, events) = channel();
        let (out, out_rx) = channel::<Option<Vec<u8>>>();
        let url = url.to_owned();
        std::thread::spawn(move || {
            let reason = run(&url, &event_tx, &out_rx).err().unwrap_or_else(|| "closed".into());
            let _ = event_tx.send(SocketEvent::Closed(reason));
        });
        Self { events, out }
    }

    pub fn send(&mut self, msg: &ClientMessage) {
        let _ = self.out.send(Some(msg.to_frame()));
    }

    pub fn poll(&mut self) -> Option<SocketEvent> {
        self.events.try_recv().ok()
    }

    pub fn close(&mut self) {
        let _ = self.out.send(None);
    }
}

fn run(url: &str, events: &Sender<SocketEvent>, out: &Receiver<Option<Vec<u8>>>) -> Result<(), String> {
    let (mut ws, _) = tungstenite::connect(url).map_err(|e| format!("{url}: {e}"))?;
    if let MaybeTlsStream::Plain(s) = ws.get_mut() {
        s.set_read_timeout(Some(Duration::from_millis(5))).map_err(|e| e.to_string())?;
        let _ = s.set_nodelay(true);
    }
    let _ = events.send(SocketEvent::Connected);
    let mut decoder = FrameDecoder::default();
    loop {
        while let Ok(frame) = out.try_recv() {
            match frame {
                Some(bytes) => ws.send(Message::Binary(bytes)).map_err(|e| e.to_string())?,
                None => {
                    let _ = ws.close(None);
                    return Ok(());
                }
            }
        }
        match ws.read() {
            Ok(Message::Binary(bytes)) => {
                decoder.push(&bytes);
                while let Some((id, body)) = decoder.next_frame().map_err(|e| e.to_string())? {
                    let msg = ServerMessage::decode(id, body).map_err(|e| format!("message {id}: {e}"))?;
                    let _ = events.send(SocketEvent::Message(Box::new(msg)));
                }
            }
            Ok(Message::Close(_)) => return Err("the gateway closed the connection".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}
