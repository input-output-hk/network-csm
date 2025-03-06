use crate::{Direction, Id};

/// CSM protocol
pub trait Protocol: Sized + Clone + Copy + std::fmt::Debug {
    /// Channel Id for this protocol
    const PROTOCOL_NUMBER: Id;

    /// Message Max size
    const MESSAGE_MAX_SIZE: usize;

    /// Message for this protocol
    type Message: cbored::Encode + cbored::Decode;

    fn transition(self, message: &Self::Message) -> Option<Self>;
    fn direction(self) -> Option<Direction>;
}
