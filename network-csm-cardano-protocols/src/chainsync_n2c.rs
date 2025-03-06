use network_csm::{Direction, Id, Protocol};

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = Id::new(5);
    const MESSAGE_MAX_SIZE: usize = 8192;

    type Message = crate::chainsync_n2n::Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        self.0.transition(message).map(Self)
    }
    fn direction(self) -> Option<Direction> {
        self.0.direction()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct State(crate::chainsync_n2n::State);

impl From<State> for crate::chainsync_n2n::State {
    fn from(s: State) -> Self {
        s.0
    }
}

impl From<crate::chainsync_n2n::State> for State {
    fn from(s: crate::chainsync_n2n::State) -> Self {
        Self(s)
    }
}
