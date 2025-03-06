use core::fmt;

use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::{format, vec::Vec};

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = Id::new(2);
    const MESSAGE_MAX_SIZE: usize = 8192;

    type Message = Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        message.can_transition(self)
    }
    fn direction(self) -> Option<Direction> {
        match self {
            State::Idle => Some(Direction::Initiator),
            State::Done => None,
            State::Intersect => Some(Direction::Responder),
            State::CanAwait => Some(Direction::Responder),
            State::MustReply => Some(Direction::Responder),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Idle,
    Done,
    Intersect,
    CanAwait,
    MustReply,
}

#[derive(Debug, Clone, CborRepr, NetworkCsmStateTransition)]
#[cborrepr(enumtype = "tagvariant")]
#[network_csm_state_transition(State,
    [
        Idle      + RequestNext          = CanAwait,
        CanAwait  + AwaitReply           = MustReply,
        CanAwait  + RollForward          = Idle,
        MustReply + RollForward          = Idle,
        CanAwait  + RollBackward         = Idle,
        MustReply + RollBackward         = Idle,
        Idle      + FindIntersect        = Intersect,
        Intersect + IntersectionFound    = Idle,
        Intersect + IntersectionNotFound = Idle,
        Idle      + SyncDone             = Done,
    ]
)]
pub enum Message {
    #[network_csm_client]
    RequestNext,
    AwaitReply,
    RollForward(CborChainsyncData, Tip),
    RollBackward(Point, Tip),
    #[network_csm_client]
    FindIntersect(Points),
    IntersectionFound(Point, Tip),
    IntersectionNotFound(Tip),
    #[network_csm_client]
    SyncDone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Point {
    Origin,
    BlockHeader { slot_nb: u64, hash: [u8; 32] },
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Point::Origin => write!(f, "origin"),
            Point::BlockHeader { slot_nb, hash } => {
                write!(f, "{}@", slot_nb)?;
                for h in hash {
                    write!(f, "{:2x}", h)?;
                }
                Ok(())
            }
        }
    }
}

impl cbored::Decode for Point {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        let array = reader
            .array()
            .map_err(cbored::DecodeErrorKind::ReaderError)
            .map_err(|e| e.context::<Self>())?;
        if array.len() == 0 {
            Ok(Point::Origin)
        } else if array.len() == 2 {
            let slot_nb = array[0]
                .decode()
                .map_err(|e| e.push_str("slot_nb").push::<Self>())?;
            let hash = array[1]
                .decode()
                .map_err(|e| e.push_str("hash").push::<Self>())?;
            Ok(Point::BlockHeader { slot_nb, hash })
        } else {
            Err(cbored::DecodeErrorKind::Custom(format!(
                "wrong expected length of 0 or 2, got {}",
                array.len()
            ))
            .context::<Self>())
        }
    }
}

impl cbored::Encode for Point {
    fn encode(&self, writer: &mut cbored::Writer) {
        match self {
            Point::Origin => {
                let len = cbored::StructureLength::from(0);
                writer.array_build(len, |_| {});
            }
            Point::BlockHeader { slot_nb, hash } => {
                let len = cbored::StructureLength::from(2);
                writer.array_build(len, |writer| {
                    writer.encode(slot_nb);
                    writer.encode(hash);
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CborChainsyncData(pub Vec<u8>);

impl cbored::Decode for CborChainsyncData {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        let cbor = reader
            .decode::<cbored::tagged::EncodedCBOR>()
            .map_err(|e| e.push::<Self>())?;
        Ok(Self(cbor.to_bytes()))
    }
}

impl cbored::Encode for CborChainsyncData {
    fn encode(&self, writer: &mut cbored::Writer) {
        writer.encode(&cbored::tagged::EncodedCBOR::from_bytes(&self.0))
    }
}

impl AsRef<[u8]> for CborChainsyncData {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct Points(pub Vec<Point>);

impl cbored::Encode for Points {
    fn encode(&self, writer: &mut cbored::Writer) {
        let len = cbored::StructureLength::from(self.0.len() as u64);
        writer.array_build(len, |writer| {
            for v in self.0.iter() {
                writer.encode(v);
            }
        })
    }
}

impl cbored::Decode for Points {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        let array = reader
            .array()
            .map_err(cbored::DecodeErrorKind::ReaderError)
            .map_err(|e| e.context::<Self>())?;
        let out = array
            .iter()
            .map(|mut r| r.decode::<Point>())
            .collect::<Result<Vec<_>, cbored::DecodeError>>()
            .map_err(|e| e.push::<Self>())?;
        Ok(Self(out))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, CborRepr)]
#[cborrepr(structure = "array")]
pub struct Tip {
    pub point: Point,
    pub block_number: u64,
}

impl fmt::Display for Tip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.block_number, self.point)
    }
}

impl Tip {
    #[allow(dead_code)]
    pub const ORIGIN: Self = Tip {
        point: Point::Origin,
        block_number: 0,
    };
}
