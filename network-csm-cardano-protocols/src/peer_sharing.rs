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
    SharePeers(Peers),
    Done,
}

#[derive(Debug, Clone, CborRepr)]
#[cborrepr(enumtype = "tagvariant")]
pub enum Peer {
    IPV4(u32, u16),
    IPV6(u32, u32, u32, u32, u16),
}

macro_rules! vec_structure {
    ($name:ident, $content:path) => {
        vec_structure!($name, $content, []);
    };
    ($name:ident, $content:path, [ $($derive_ident:ident)* ]) => {
        #[derive(Clone, Debug, $($derive_ident),*)]
        pub struct $name(pub Vec<$content>);

        impl AsRef<[$content]> for $name {
            fn as_ref(&self) -> &[$content] {
                &self.0
            }
        }

        impl From<Vec<$content>> for $name {
            fn from(v: Vec<$content>) -> Self {
                Self(v)
            }
        }

        impl $name {
            pub fn len(&self) -> usize {
                self.0.len()
            }

            pub fn new() -> Self {
                Self(Vec::new())
            }

            pub fn iter(&self) -> impl Iterator<Item = &$content> {
                self.0.iter()
            }

            pub fn into_iter(self) -> impl Iterator<Item = $content> {
                self.0.into_iter()
            }

            pub fn push(&mut self, t: $content) {
                self.0.push(t)
            }
        }

        impl ::cbored::Encode for $name {
            fn encode(&self, writer: &mut ::cbored::Writer) {
                let len = ::cbored::StructureLength::from(self.0.len() as u64);
                writer.array_build(len, |writer| {
                    for v in self.0.iter() {
                        writer.encode(v);
                    }
                })
            }
        }

        impl ::cbored::Decode for $name {
            fn decode<'a>(
                reader: &mut ::cbored::Reader<'a>,
            ) -> Result<Self, ::cbored::DecodeError> {
                let array = reader
                    .array()
                    .map_err(::cbored::DecodeErrorKind::ReaderError)
                    .map_err(|e| e.context::<Self>())?;
                let out = array
                    .iter()
                    .map(|mut r| r.decode::<$content>())
                    .collect::<Result<Vec<_>, ::cbored::DecodeError>>()
                    .map_err(|e| e.push::<Self>())?;
                Ok($name(out))
            }
        }
    };
}

vec_structure!(Peers, Peer);
