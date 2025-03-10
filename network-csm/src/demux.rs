use core::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::frame::{HEADER_SIZE, Header};

/// CSM Demuxer
pub struct Demux {
    frame_state: DemuxState,
    /// Number of bytes read by the demuxer
    pub bytes_read: Arc<AtomicU64>,
}

impl Demux {
    /// Create a new Demux
    pub fn new() -> Self {
        Self {
            frame_state: DemuxState::new(),
            bytes_read: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Input some data into the Demux
    ///
    /// Return the number of bytes consumed and the demux result
    pub fn ingress<'a>(&mut self, data: &'a [u8]) -> (usize, DemuxResult<'a>) {
        let (sz, r) = self.frame_state.process(data);
        self.bytes_read.fetch_add(sz as u64, Ordering::Relaxed);
        (sz, r)
    }
}

#[derive(Debug)]
pub enum DemuxState {
    Header([u8; HEADER_SIZE], u32),
    Content(Header, NonZeroUsize),
}

pub enum DemuxResult<'a> {
    /// No event trigger
    Continue,
    /// A fully formed Header has been received with time, id and responder
    HeaderReceived(Header),
    /// Data to append for a specific id, it also include if this result in a finished data packet
    DataAppend(Header, bool, &'a [u8]),
}

impl DemuxState {
    fn new() -> Self {
        Self::Header([0; 8], 0)
    }

    /// Process bytes into the frame state
    ///
    /// header data are just appended directly into the state, and might trigger a HeaderReceived when the header is fully received
    ///
    /// frame data is returned to the caller after the state is updated to take in consideration the data
    ///
    /// The function returns the number of bytes that were processed, which may be inferior to the size of the data
    fn process<'a>(&mut self, data: &'a [u8]) -> (usize, DemuxResult<'a>) {
        if data.is_empty() {
            return (0, DemuxResult::Continue);
        }
        tracing::debug!("demux data={} current-state={:?}", data.len(), self);
        match self {
            DemuxState::Header(buf, current_state) => {
                let current = *current_state as usize;
                let bytes = data.len().min(HEADER_SIZE - current);
                // is it enough to finish the header ?
                if data.len() >= HEADER_SIZE - current {
                    buf[current..current + bytes].copy_from_slice(&data[..bytes]);
                    let header = Header::from_bytes(*buf);
                    let len = header.payload_length() as usize;
                    let Some(len) = NonZeroUsize::new(len) else {
                        // frame with no content, just set the state to a new header directly
                        *self = DemuxState::new();
                        return (bytes, DemuxResult::HeaderReceived(header));
                    };

                    *self = DemuxState::Content(header, len);
                    (bytes, DemuxResult::HeaderReceived(header))
                } else {
                    buf[current..current + bytes].copy_from_slice(data);
                    *current_state = (current + bytes) as u32;
                    (data.len(), DemuxResult::Continue)
                }
            }
            DemuxState::Content(header, rem) => {
                // check if it is enough to finish the content
                let finished = rem.get() <= data.len();

                let header = header.clone();
                if rem.get() <= data.len() {
                    let callback_data = &data[0..rem.get()];
                    let processed = rem.get();
                    *self = DemuxState::new();
                    (
                        processed,
                        DemuxResult::DataAppend(header, finished, callback_data),
                    )
                } else {
                    *rem = NonZeroUsize::new(rem.get() - data.len()).unwrap();
                    (data.len(), DemuxResult::DataAppend(header, finished, data))
                }
            }
        }
    }
}
