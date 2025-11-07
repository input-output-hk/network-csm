use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;
use std::fmt;

use alloc::{format, vec::Vec};

pub use crate::chainsync_n2n::Point;
use crate::protocol_numbers;

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = protocol_numbers::BLOCKFETCH;
    const MESSAGE_MAX_SIZE: usize = 2_500 * 1_024;

    type Message = Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        message.can_transition(self)
    }
    fn direction(self) -> Option<Direction> {
        match self {
            State::Idle => Some(Direction::Initiator),
            State::Done => None,
            State::Busy => Some(Direction::Responder),
            State::Streaming => Some(Direction::Responder),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Idle,
    Done,
    Busy,
    Streaming,
}

#[derive(Debug, Clone, CborRepr, NetworkCsmStateTransition)]
#[cborrepr(enumtype = "tagvariant")]
#[network_csm_state_transition(State,
    [
        Idle + RequestRange = Busy,
        Idle + ClientDone = Done,
        Busy + NoBlocks = Idle,
        Busy + StartBatch = Streaming,
        Streaming + Block = Streaming,
        Streaming + BatchDone = Idle,
    ]
)]
pub enum Message {
    #[network_csm_client]
    RequestRange(Point, Point),
    ClientDone,
    StartBatch,
    NoBlocks,
    Block(CborBlockData),
    BatchDone,
}

#[derive(Clone)]
pub struct CborBlockData(pub Vec<u8>);

impl fmt::Debug for CborBlockData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CborBlockData")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

impl cbored::Decode for CborBlockData {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        let cbor = reader
            .decode::<cbored::tagged::EncodedCBOR>()
            .map_err(|e| e.push::<Self>())?;
        Ok(Self(cbor.to_bytes()))
    }
}

impl cbored::Encode for CborBlockData {
    fn encode(&self, writer: &mut cbored::Writer) {
        writer.encode(&cbored::tagged::EncodedCBOR::from_bytes(&self.0))
    }
}

impl AsRef<[u8]> for CborBlockData {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
