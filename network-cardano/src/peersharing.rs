use anyhow::{Result, anyhow};
use network_csm_cardano_protocols::peer_sharing::{Message, Peer, State};
use network_csm_tokio::AsyncChannel;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};

/// Client wrapper for the PeerSharing mini-protocol (initiator side).
pub struct PeerSharingClient(AsyncChannel<State>);

impl PeerSharingClient {
    pub fn new(channel: AsyncChannel<State>) -> Self {
        Self(channel)
    }

    /// Send one ShareRequest and wait briefly for a reply.
    /// Returns unique peers as `SocketAddr`s.
    pub async fn request_once(&mut self) -> Result<Vec<SocketAddr>> {
        self.0.write_one(Message::ShareRequest(5)).await;

        let msg = timeout(Duration::from_secs(5), self.0.read_one())
            .await
            .map_err(|_| anyhow!("PeerSharing timed out (no reply)"))?
            .map_err(|e| anyhow!("PeerSharing read error: {e:?}"))?;

        let peers_raw = match msg {
            Message::SharePeers(list) => list,
            other => {
                warn!("Unexpected PeerSharing message: {:?}", other);
                Vec::new()
            }
        };

        let mut out: HashSet<SocketAddr> = HashSet::new();
        for p in peers_raw {
            if let Some(sa) = peer_to_socketaddr(&p) {
                out.insert(sa);
            }
        }
        let v = out.into_iter().collect::<Vec<_>>();
        info!("PeerSharing returned {} peers", v.len());
        Ok(v)
    }
}

fn peer_to_socketaddr(peer: &Peer) -> Option<SocketAddr> {
    match *peer {
        // Peer::IPV4 carries a u32
        Peer::IPV4(bits, port) => {
            let ip = Ipv4Addr::from(bits);
            Some(SocketAddr::new(IpAddr::V4(ip), port))
        }
        // Peer::IPV6 is 4x u32 + port; split into 8x u16 and build an IPv6 address
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
            Some(SocketAddr::new(IpAddr::V6(ip), port))
        }
    }
}
