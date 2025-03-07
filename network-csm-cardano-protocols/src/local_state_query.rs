use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::format;

pub use crate::chainsync_n2n::Point;

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = Id::new(7);
    const MESSAGE_MAX_SIZE: usize = 8192;

    type Message = Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        message.can_transition(self)
    }
    fn direction(self) -> Option<Direction> {
        match self {
            State::Idle => Some(Direction::Initiator),
            State::Acquiring => Some(Direction::Responder),
            State::Acquired => Some(Direction::Initiator),
            State::Querying => Some(Direction::Responder),
            State::Done => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Idle,
    Acquiring,
    Acquired,
    Querying,
    Done,
}

#[derive(Debug, Clone, CborRepr, NetworkCsmStateTransition)]
#[cborrepr(enumtype = "tagvariant")]
#[network_csm_state_transition(State,
    [
        Idle + Acquire = Acquiring,
        Acquiring + Acquired = Acquired,
        Acquiring + Acquire2 = Acquired,
        Acquiring + Acquire3 = Acquired,
        Acquired + Query = Querying,
        Querying + Result = Acquired,
        Acquired + ReAcquire = Acquiring,
        Acquired + ReAcquire2 = Acquiring,
        Acquired + ReAcquire3 = Acquiring,
        Acquiring + Failure = Idle,
        Acquired + Release = Idle,
        Idle + Done = Done,
    ]
)]
pub enum Message {
    #[network_csm_client]
    Acquire(Point),
    Acquired,
    Failure(Failure),
    Query(cbored::DataOwned),
    Result(cbored::DataOwned),
    Release,
    ReAcquire(Point),
    Done,
    Acquire2,
    ReAcquire2,
    Acquire3,
    ReAcquire3,
}

#[derive(Debug, Clone, Copy, CborRepr)]
#[cborrepr(enumtype = "enumint")]
pub enum Failure {
    PointTooOld,
    PointNotOnChain,
}
