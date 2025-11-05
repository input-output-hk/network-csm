use network_csm_tokio::MessageError;

use network_csm_cardano_protocols::peer_sharing::{Message, Peer, State};
use network_csm_tokio::AsyncChannel;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use tracing::{debug, warn};

/// Client wrapper for the PeerSharing mini-protocol (initiator side).
pub struct PeerSharingClient(AsyncChannel<State>);

impl PeerSharingClient {
    pub fn new(channel: AsyncChannel<State>) -> Self {
        Self(channel)
    }

    /// Send one ShareRequest and wait briefly for a reply.
    /// Returns unique peers as `SocketAddr`s.
    pub async fn request_once(
        &mut self,
        count: u8,
    ) -> Result<Vec<SocketAddr>, MessageError<State>> {
        self.0.write_one(Message::ShareRequest(count as u8)).await;

        let msg = self.0.read_one().await?;

        let peers_raw = match msg {
            Message::SharePeers(list) => list,
            other => {
                warn!("Unexpected PeerSharing message: {:?}", other);
                Vec::new()
            }
        };

        let mut out: HashSet<SocketAddr> = HashSet::new();
        for p in peers_raw {
            out.insert(p.to_socketaddr());
        }

        let v = out.into_iter().collect::<Vec<_>>();
        debug!("PeerSharing returned {} peers", v.len());

        Ok(v)
    }
}

trait ToSocketAddr {
    fn to_socketaddr(&self) -> SocketAddr;
}

impl ToSocketAddr for Peer {
    fn to_socketaddr(&self) -> SocketAddr {
        match *self {
            Peer::IPV4(bits, port) => {
                let ip = Ipv4Addr::from(bits);
                SocketAddr::new(IpAddr::V4(ip), port)
            }
            Peer::IPV6(w0, w1, w2, w3, port) => {
                let parts = [
                    (w0 >> 16) as u16,
                    (w0 & 0xFFFF) as u16,
                    (w1 >> 16) as u16,
                    (w1 & 0xFFFF) as u16,
                    (w2 >> 16) as u16,
                    (w2 & 0xFFFF) as u16,
                    (w3 >> 16) as u16,
                    (w3 & 0xFFFF) as u16,
                ];
                let ip = Ipv6Addr::new(
                    parts[0], parts[1], parts[2], parts[3], parts[4], parts[5], parts[6], parts[7],
                );
                SocketAddr::new(IpAddr::V6(ip), port)
            }
        }
    }
}
