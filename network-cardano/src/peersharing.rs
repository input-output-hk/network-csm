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
    /// Returns unique peers as SocketAddrs.
    pub async fn request_once(
        &mut self,
        count: u8,
    ) -> Result<Vec<SocketAddr>, MessageError<State>> {
        // Send one ShareRequest
        self.0.write_one(Message::ShareRequest(count)).await;

        // Await a reply
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
