//! CBOR Simple Multiplexer Network Library
#![deny(unsafe_code)]

extern crate alloc;

mod buf;
mod cbor_helper;
mod channel;
mod channels_map;
mod demux;
mod frame;
mod mux;
mod protocol;

pub use cbor_helper::{CborBufValidate, cbor_buf_validate};

pub use channel::{Channel, ReadMessageError};
pub use channels_map::{ChannelsMap, ChannelsMapBuilder, DuplicateChannel};
pub use demux::{Demux, DemuxResult};
pub use frame::{Direction, HEADER_SIZE, Header, Id, OnDirection, Time};
pub use mux::Mux;
pub use protocol::Protocol;
