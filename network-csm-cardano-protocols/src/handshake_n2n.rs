//! this is the only builtin protocol
use cbored::{CborRepr, Positive};
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::{format, string::String, vec::Vec};

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Propose,
    Confirm,
    Done,
}

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = Id::ZERO;

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
    pub diffusion: DiffusionMode,
    pub peer_sharing: PeerSharing,
    pub query: bool,
}

#[derive(Debug, Clone, CborRepr, PartialEq, Eq)]
#[cborrepr(enumtype = "tagvariant")]
pub enum RefuseReason {
    VersionMismatch(Versions),
    HandshakeDecodeError(Version, String),
    Refused(Version, String),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffusionMode {
    InitiatorOnly,
    InitiatorAndResponder,
}

impl cbored::Encode for DiffusionMode {
    fn encode(&self, writer: &mut cbored::Writer) {
        match self {
            DiffusionMode::InitiatorOnly => writer.bool(false),
            DiffusionMode::InitiatorAndResponder => writer.bool(true),
        }
    }
}

impl cbored::Decode for DiffusionMode {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        match reader
            .bool()
            .map_err(cbored::DecodeErrorKind::ReaderError)
            .map_err(|e| e.context::<Self>())?
        {
            true => Ok(DiffusionMode::InitiatorAndResponder),
            false => Ok(DiffusionMode::InitiatorOnly),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerSharing {
    Disabled,
    Enabled,
}

impl cbored::Encode for PeerSharing {
    fn encode(&self, writer: &mut cbored::Writer) {
        match self {
            PeerSharing::Disabled => writer.positive(Positive::canonical(0)),
            PeerSharing::Enabled => writer.positive(Positive::canonical(1)),
        }
    }
}

impl cbored::Decode for PeerSharing {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        match reader
            .positive()
            .map_err(cbored::DecodeErrorKind::ReaderError)
            .map_err(|e| e.context::<Self>())?
            .to_u64()
        {
            0 => Ok(PeerSharing::Disabled),
            1 => Ok(PeerSharing::Enabled),
            v => Err(
                cbored::DecodeErrorKind::Custom(format!("unknown peer sharing : {}", v))
                    .context::<Self>(),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CborRepr)]
#[cborrepr(structure = "flat")]
pub struct Magic(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u64)]
pub enum Version {
    V6 = 0x6,
    V7 = 0x7,
    V8 = 0x8,
    V9 = 0x9,
    V10 = 0xa,
    V11 = 0xb,
    V13 = 0xd,
    V14 = 0xe,
}

impl Version {
    pub const KNOWN: [Self; 8] = [
        Version::V6,
        Version::V7,
        Version::V8,
        Version::V9,
        Version::V10,
        Version::V11,
        Version::V13,
        Version::V14,
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
