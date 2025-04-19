//! this is the only builtin protocol
use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use crate::protocol_numbers;

pub use super::handshake_n2n::Magic;
pub use super::handshake_n2n::RefuseReason;

use alloc::{format, vec::Vec};

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Propose,
    Confirm,
    Done,
}

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = protocol_numbers::HANDSHAKE;

    const MESSAGE_MAX_SIZE: usize = 2048;

    type Message = Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        message.can_transition(self)
    }
    fn direction(self) -> Option<Direction> {
        match self {
            State::Propose => Some(Direction::Initiator),
            State::Confirm => Some(Direction::Responder),
            State::Done => None,
        }
    }
}

#[derive(Clone, Debug, CborRepr, PartialEq, Eq, NetworkCsmStateTransition)]
#[cborrepr(enumtype = "tagvariant")]
#[network_csm_state_transition(State,
    [
        Propose + ProposeVersions = Confirm,
        Confirm + AcceptVersion   = Done,
        Confirm + Refuse          = Done,
        Confirm + QueryReply      = Done,
    ]
)]
pub enum Message {
    #[network_csm_client]
    ProposeVersions(VersionProposal),
    AcceptVersion(Version, HandshakeNodeData),
    Refuse(RefuseReason),
    QueryReply(VersionProposal),
}

#[derive(Debug, Clone, CborRepr, PartialEq, Eq)]
#[cborrepr(structure = "array")]
pub struct HandshakeNodeData {
    pub magic: Magic,
    pub query: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionProposal(pub Vec<(Version, HandshakeNodeData)>);

impl VersionProposal {
    pub fn propose_data(self) -> Vec<u8> {
        let handshake_msg = Message::ProposeVersions(self);
        let mut writer = cbored::Writer::new();
        writer.encode(&handshake_msg);
        writer.finalize()
    }
}

impl cbored::Encode for VersionProposal {
    fn encode(&self, writer: &mut cbored::Writer) {
        let len = cbored::StructureLength::from(self.0.len() as u64);
        writer.map_build(len, |writer| {
            for (k, v) in self.0.iter() {
                writer.encode(k);
                writer.encode(v);
            }
        })
    }
}

impl cbored::Decode for VersionProposal {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        let map = reader
            .map()
            .map_err(cbored::DecodeErrorKind::ReaderError)
            .map_err(|e| e.context::<Self>())?;
        let out = map
            .iter()
            .map(|(mut k, mut v)| {
                let key = k.decode().map_err(|e| e.push_str("key").push::<Self>())?;
                let value = v.decode().map_err(|e| e.push_str("val").push::<Self>())?;
                Ok((key, value))
            })
            .collect::<Result<Vec<(Version, HandshakeNodeData)>, cbored::DecodeError>>()?;
        Ok(Self(out))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versions(pub Vec<Version>);

impl cbored::Encode for Versions {
    fn encode(&self, writer: &mut cbored::Writer) {
        let len = cbored::StructureLength::from(self.0.len() as u64);
        writer.array_build(len, |writer| {
            for v in self.0.iter() {
                writer.encode(v);
            }
        })
    }
}

impl cbored::Decode for Versions {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        let array = reader
            .array()
            .map_err(cbored::DecodeErrorKind::ReaderError)
            .map_err(|e| e.context::<Self>())?;
        let out = array
            .iter()
            .map(|mut r| r.decode::<Version>())
            .collect::<Result<Vec<_>, cbored::DecodeError>>()
            .map_err(|e| e.push::<Self>())?;
        Ok(Self(out))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u64)]
pub enum Version {
    V16 = 32784,
    V17 = 32785,
    V18 = 32786,
    V19 = 32787,
    V20 = 32788,
}

impl Version {
    pub const KNOWN: [Self; 5] = [
        Version::V16,
        Version::V17,
        Version::V18,
        Version::V19,
        Version::V20,
    ];

    pub fn from_integer(v: u64) -> Option<Version> {
        for k in Self::KNOWN {
            if k as u64 == v {
                return Some(k);
            }
        }
        None
    }
}

impl cbored::Decode for Version {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        let ver = reader.decode()?;
        Version::from_integer(ver).ok_or(
            cbored::DecodeErrorKind::Custom(format!("unknown version : {}", ver)).context::<Self>(),
        )
    }
}

impl cbored::Encode for Version {
    fn encode(&self, writer: &mut cbored::Writer) {
        writer.encode(&(*self as u64))
    }
}
