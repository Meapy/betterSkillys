//! Framing: each message is a 4-byte big-endian length (counting these 5 header bytes), a 1-byte
//! message id, then the body. There is no encryption.

use crate::messages::{ClientMessage, ServerMessage};
use crate::wire::Writer;

pub const HEADER: usize = 5;

/// Largest frame accepted from the server. The biggest real ones are Updates of a whole map
/// region, well under a megabyte.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("bad frame length {0}")]
    BadLength(i64),
}

fn frame(id: u8, encode: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = Writer { buf: vec![0; HEADER] };
    encode(&mut w);
    let len = w.buf.len() as u32;
    w.buf[..4].copy_from_slice(&len.to_be_bytes());
    w.buf[4] = id;
    w.buf
}

impl ClientMessage {
    /// The whole frame, ready to write to the socket.
    pub fn to_frame(&self) -> Vec<u8> {
        frame(self.id(), |w| self.encode_body(w))
    }
}

impl ServerMessage {
    /// The whole frame (for tests and the test server).
    pub fn to_frame(&self) -> Vec<u8> {
        frame(self.id(), |w| self.encode_body(w))
    }
}

/// Collects bytes as they arrive and splits them into frames.
#[derive(Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
    start: usize,
}

impl FrameDecoder {
    pub fn push(&mut self, bytes: &[u8]) {
        if self.start > 0 && self.start == self.buf.len() {
            self.buf.clear();
            self.start = 0;
        }
        self.buf.extend_from_slice(bytes);
    }

    /// The next complete frame as (id, body), if one has arrived.
    pub fn next_frame(&mut self) -> Result<Option<(u8, &[u8])>, FrameError> {
        let pending = &self.buf[self.start..];
        if pending.len() < 4 {
            return Ok(None);
        }
        let len = i32::from_be_bytes(pending[..4].try_into().unwrap()) as i64;
        if len < HEADER as i64 || len > MAX_FRAME as i64 {
            return Err(FrameError::BadLength(len));
        }
        let len = len as usize;
        if pending.len() < len {
            // Compact before the buffer grows: drop what's been consumed.
            if self.start > 0 && self.start >= self.buf.len() / 2 {
                self.buf.drain(..self.start);
                self.start = 0;
            }
            return Ok(None);
        }
        let begin = self.start;
        self.start += len;
        let id = self.buf[begin + 4];
        Ok(Some((id, &self.buf[begin + HEADER..begin + len])))
    }
}
