//! lowlevel types for the mux and demux framing

use core::fmt;

/// Header size for the CSM packet
pub const HEADER_SIZE: usize = 8;

const ID_MASK: u16 = 0x7f_ff;

#[derive(Clone, Debug)]
pub struct Time(pub u32);

impl Time {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn now() -> Self {
        Time(
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u32,
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub fn now() -> Self {
        Time(0)
    }
}

/// Channel ID
///
/// Behind the scene, it's a 15 bits unsigned integer
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id(u16);

impl Id {
    /// The ZERO Id (reserved for a mandatory channel for handshaking)
    pub const ZERO: Self = Id(0);

    /// Create a new Id from a raw integer
    ///
    /// Might panic if the channel is greater than 2^15
    pub const fn new(v: u16) -> Self {
        assert!(v < 0x8000);
        Self(v)
    }

    /// Get the associated integer
    pub const fn int(self) -> u16 {
        self.0
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Initiator,
    Responder,
}

impl Direction {
    pub const fn as_int(self) -> u64 {
        match self {
            Direction::Initiator => 0,
            Direction::Responder => 1,
        }
    }
}

impl std::ops::Not for Direction {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Direction::Initiator => Direction::Responder,
            Direction::Responder => Direction::Initiator,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Header(u64);

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "time={:?},id={:?},d={},len={}",
            self.time(),
            self.id(),
            if self.is_responder() { 'R' } else { 'S' },
            self.payload_length(),
        )
    }
}

impl Header {
    /// Create a new Header for muxer/demuxer
    pub const fn new(time: Time, id: Id, direction: Direction, payload_length: u16) -> Self {
        let r = direction.as_int() << 31;
        let v = ((time.0 as u64) << 32) | ((id.0 as u64) << 16) | r | (payload_length as u64);
        Self(v)
    }

    /// Serialize the header into a 8 bytes structure
    pub const fn to_bytes(self) -> [u8; HEADER_SIZE] {
        self.0.to_be_bytes()
    }

    /// Parse 8 bytes into a Header
    pub const fn from_bytes(bytes: [u8; HEADER_SIZE]) -> Self {
        Self(u64::from_be_bytes(bytes))
    }

    pub const fn time(self) -> Time {
        Time((self.0 >> 32) as u32)
    }

    pub const fn id(self) -> Id {
        Id((self.0 >> 16) as u16 & ID_MASK)
    }

    pub const fn direction(self) -> Direction {
        if self.is_initiator() {
            Direction::Initiator
        } else {
            Direction::Responder
        }
    }

    pub const fn is_initiator(self) -> bool {
        ((self.0 >> 31) & 0x1) == 0x0
    }

    pub const fn is_responder(self) -> bool {
        ((self.0 >> 31) & 0x1) == 0x1
    }

    pub const fn payload_length(self) -> u16 {
        (self.0 & 0xffff) as u16
    }
}

#[derive(Clone)]
pub enum OnDirection<T> {
    Initiator(T),
    Responder(T),
    InitiatorAndResponder(T, T),
}

impl<T> OnDirection<T> {
    pub fn on_initiator<F, V>(&self, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        match self {
            OnDirection::Initiator(t) => Some(f(t)),
            OnDirection::Responder(_) => None,
            OnDirection::InitiatorAndResponder(t, _) => Some(f(t)),
        }
    }
    pub fn on_responder<F, V>(&self, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        match self {
            OnDirection::Initiator(_) => None,
            OnDirection::Responder(t) => Some(f(t)),
            OnDirection::InitiatorAndResponder(_, t) => Some(f(t)),
        }
    }

    pub fn map<F, U>(&self, f: F) -> OnDirection<U>
    where
        F: Fn(&T) -> U,
    {
        match self {
            OnDirection::Initiator(t) => OnDirection::Initiator(f(t)),
            OnDirection::Responder(t) => OnDirection::Responder(f(t)),
            OnDirection::InitiatorAndResponder(t1, t2) => {
                OnDirection::InitiatorAndResponder(f(t1), f(t2))
            }
        }
    }

    pub fn into_split(self) -> (Option<T>, Option<T>) {
        match self {
            OnDirection::Initiator(t) => (Some(t), None),
            OnDirection::Responder(t) => (None, Some(t)),
            OnDirection::InitiatorAndResponder(t1, t2) => (Some(t1), Some(t2)),
        }
    }

    pub fn split(&self) -> (Option<&T>, Option<&T>) {
        match self {
            OnDirection::Initiator(t) => (Some(t), None),
            OnDirection::Responder(t) => (None, Some(t)),
            OnDirection::InitiatorAndResponder(t1, t2) => (Some(t1), Some(t2)),
        }
    }

    pub fn has_direction(&self, dir: Direction) -> bool {
        match self {
            OnDirection::Initiator(_) => dir == Direction::Initiator,
            OnDirection::Responder(_) => dir == Direction::Responder,
            OnDirection::InitiatorAndResponder(_, _) => true,
        }
    }

    pub fn get(&self, dir: Direction) -> Option<&T> {
        match (self, dir) {
            (OnDirection::Initiator(t), Direction::Initiator) => Some(t),
            (OnDirection::Responder(t), Direction::Responder) => Some(t),
            (OnDirection::InitiatorAndResponder(t, _), Direction::Initiator) => Some(t),
            (OnDirection::InitiatorAndResponder(_, t), Direction::Responder) => Some(t),
            _ => None,
        }
    }
}

impl OnDirection<()> {
    pub const INITIATOR: OnDirection<()> = OnDirection::Initiator(());
    pub const RESPONDER: OnDirection<()> = OnDirection::Responder(());
    pub const INITIATOR_AND_RESPONDER: OnDirection<()> = OnDirection::InitiatorAndResponder((), ());
}
