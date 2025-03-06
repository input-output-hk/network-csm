use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::buf::Buf;
use crate::frame::{HEADER_SIZE, Time};
use crate::{Direction, Header, Id};

/// Multiplexer state
pub struct Mux {
    buffer: Buf,
    /// Number of bytes written to this multiplexer
    pub bytes_written: Arc<AtomicU64>,
}

impl Mux {
    /// Create a new Mux with a specified buffer size
    pub fn new(size: usize) -> Self {
        Self {
            bytes_written: Arc::new(AtomicU64::new(0)),
            buffer: Buf::new(size),
        }
    }

    pub fn egress<'a>(&mut self, id: Id, direction: Direction, data: &'a [u8]) -> Result<(), ()> {
        tracing::info!(
            "egress id={:?} direction={:?} data={}",
            id,
            direction,
            data.len()
        );
        let Ok(payload_length) = u16::try_from(data.len()) else {
            return Err(());
        };
        let header = Header::new(Time::now(), id, direction, payload_length);
        self.bytes_written
            .fetch_add(HEADER_SIZE as u64 + data.len() as u64, Ordering::Relaxed);
        self.buffer.append_atomic2(&header.to_bytes(), data)
    }

    pub fn work(&self) -> &[u8] {
        self.buffer.available()
    }

    /// Return the writable bytes for this buffer
    pub fn writable(&mut self) -> &mut [u8] {
        self.buffer.empty_mut()
    }

    pub fn consume(&mut self, bytes: usize) {
        self.buffer.consume(bytes)
    }
}
