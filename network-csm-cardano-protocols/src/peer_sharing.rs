use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::{format, vec::Vec};

pub use crate::chainsync_n2n::Point;

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = Id::new(10);
    const MESSAGE_MAX_SIZE: usize = 8192;

    type Message = Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        message.can_transition(self)
    }
    fn direction(self) -> Option<Direction> {
        match self {
            State::Idle => Some(Direction::Initiator),
            State::Busy => Some(Direction::Responder),
            State::Done => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Idle,
    Busy,
    Done,
}

#[derive(Debug, Clone, CborRepr, NetworkCsmStateTransition)]
#[cborrepr(enumtype = "tagvariant")]
#[network_csm_state_transition(State,
    [
        Idle + ShareRequest = Busy,
        Busy + SharePeers = Idle,
        Idle + Done = Done,
    ]
)]
pub enum Message {
    #[network_csm_client]
    ShareRequest(u8),
    SharePeers(Vec<Peer>),
    Done,
}

#[derive(Debug, Clone, CborRepr)]
#[cborrepr(enumtype = "tagvariant")]
pub enum Peer {
    IPV4(u32, u16),
    IPV6(u32, u32, u32, u32, u16),
}
