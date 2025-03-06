use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::format;

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = Id::new(8);
    const MESSAGE_MAX_SIZE: usize = 64;

    type Message = Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        message.can_transition(self)
    }
    fn direction(self) -> Option<Direction> {
        match self {
            State::Client => Some(Direction::Initiator),
            State::Server => Some(Direction::Responder),
            State::Done => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Client,
    Server,
    Done,
}

#[derive(Debug, Clone, CborRepr, NetworkCsmStateTransition)]
#[cborrepr(enumtype = "tagvariant")]
#[network_csm_state_transition(State,
[
        Client + KeepAlive         = Server,
        Client + Done              = Done,
        Server + KeepAliveResponse = Done,
    ]
)]
pub enum Message {
    #[network_csm_client]
    KeepAlive(u16),
    KeepAliveResponse(u16),
    #[network_csm_client]
    Done,
}
