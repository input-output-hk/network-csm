use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::format;

pub use crate::chainsync_n2n::Point;

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = Id::new(6);
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
        Idle + SubmitTx = Busy,
        Busy + AcceptTx = Idle,
        Busy + RejectTx = Idle,
        Idle + Done = Done,
    ]
)]
pub enum Message {
    #[network_csm_client]
    SubmitTx(crate::tx_submission::Tx),
    AcceptTx,
    RejectTx(u64),
    Done,
}
