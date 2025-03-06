use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::{format, vec::Vec};

pub use crate::chainsync_n2n::Point;

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = Id::new(3);
    const MESSAGE_MAX_SIZE: usize = 8192;

    type Message = Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        message.can_transition(self)
    }
    fn direction(self) -> Option<Direction> {
        match self {
            State::Idle => Some(Direction::Responder),
            State::Done => None,
            State::Init => Some(Direction::Initiator),
            State::Txs => Some(Direction::Initiator),
            State::TxIdsBlocking => Some(Direction::Initiator),
            State::TxIdsNonBlocking => Some(Direction::Initiator),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Init,
    Idle,
    Txs,
    TxIdsBlocking,
    TxIdsNonBlocking,
    Done,
}

#[derive(Debug, Clone, CborRepr, NetworkCsmStateTransition)]
#[cborrepr(enumtype = "tagvariant")]
#[network_csm_state_transition(State,
    [
        Init + Init = Idle,
        Idle + RequestTxIds = TxIdsBlocking,
        Idle + RequestTxs = Txs,
        Txs + ReplyTxs = Idle,
        TxIdsNonBlocking + ReplyTxIds = Idle,
        TxIdsBlocking + Done = Done,
        TxIdsBlocking + ReplyTxIds = Idle,
    ]
)]
pub enum Message {
    #[network_csm_client]
    Init,
    RequestTxIds(bool, u16, u16),
    //#[network_csm_client]
    ReplyTxIds(Vec<TxIdAndSize>),
    RequestTxs(Vec<TxId>),
    #[network_csm_client]
    ReplyTxs(Vec<Tx>),
    Done,
}

#[derive(Debug, Clone, CborRepr)]
#[cborrepr(structure = "array")]
pub struct TxIdAndSize {
    id: TxId,
    size: u32,
}

#[derive(Debug, Clone)]
pub struct TxId(Vec<u8>);

#[derive(Debug, Clone)]
pub struct Tx(Vec<u8>);

impl cbored::Decode for TxId {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        reader.decode().map(Self)
    }
}

impl cbored::Encode for TxId {
    fn encode(&self, writer: &mut cbored::Writer) {
        writer.encode(&self.0[..])
    }
}

impl cbored::Decode for Tx {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        reader.decode().map(Self)
    }
}

impl cbored::Encode for Tx {
    fn encode(&self, writer: &mut cbored::Writer) {
        writer.encode(&self.0[..])
    }
}
