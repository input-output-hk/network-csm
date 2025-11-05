use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::{format, vec::Vec};

pub use crate::chainsync_n2n::Point;
use crate::protocol_numbers;

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = protocol_numbers::PEER_SHARING;
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
    SharePeers(Vec<Peer>),
    Done,
}

#[derive(Debug, Clone, CborRepr)]
#[cborrepr(enumtype = "tagvariant")]
pub enum Peer {
    IPV4(u32, u16),
    IPV6(u32, u32, u32, u32, u16),
}
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

impl Peer {
    pub fn to_socketaddr(&self) -> SocketAddr {
        match *self {
            Peer::IPV4(bits, port) => {
                let ip = Ipv4Addr::from(bits);
                SocketAddr::new(IpAddr::V4(ip), port)
            }
            Peer::IPV6(w0, w1, w2, w3, port) => {
                let parts = [
                    (w0 >> 16) as u16, (w0 & 0xFFFF) as u16,
                    (w1 >> 16) as u16, (w1 & 0xFFFF) as u16,
                    (w2 >> 16) as u16, (w2 & 0xFFFF) as u16,
                    (w3 >> 16) as u16, (w3 & 0xFFFF) as u16,
                ];
                let ip = Ipv6Addr::new(
                    parts[0], parts[1], parts[2], parts[3],
                    parts[4], parts[5], parts[6], parts[7],
                );
                SocketAddr::new(IpAddr::V6(ip), port)
            }
        }
    }
}
