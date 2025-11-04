use crate::{
    BlockFetchClient, ChainSyncClient,
    handshake::{HandshakeN2CClient, HandshakeN2NClient},
    peersharing::PeerSharingClient,
};

use network_csm::DuplicateChannel;
use network_csm_cardano_protocols::{handshake_n2c, handshake_n2n, protocol_numbers};
use network_csm_tokio::{Handle, HandleChannels};
use tokio::io::{AsyncRead, AsyncWrite};

use super::ConnectionError;

/// [`ClientBuilder`] to establish a client connection with a remote
/// peer.
///
pub struct ClientBuilder {
    channels: HandleChannels,
}

pub struct Client {
    #[allow(unused)]
    handle: Handle,
}

impl ClientBuilder {
    pub fn new() -> Self {
        let channels = HandleChannels::new();
        Self { channels }
    }

    pub fn with_n2n_chainsync(&mut self) -> Result<ChainSyncClient, DuplicateChannel> {
        self.channels.add_initiator().map(ChainSyncClient::new_n2n)
    }

    pub fn with_n2c_chainsync(&mut self) -> Result<ChainSyncClient, DuplicateChannel> {
        self.channels.add_initiator().map(ChainSyncClient::new_n2n)
    }

    pub fn with_blockfetch(&mut self) -> Result<BlockFetchClient, DuplicateChannel> {
        self.channels.add_initiator().map(BlockFetchClient::new)
    }

   pub fn with_peersharing(&mut self) -> std::result::Result<PeerSharingClient, DuplicateChannel> {
        self.channels.add_initiator().map(PeerSharingClient::new)
    }

     
    


    pub(crate) async fn build_n2n<R, W>(
        mut self,
        read_stream: R,
        write_stream: W,
        version: handshake_n2n::Version,
        magic: handshake_n2n::Magic,
    ) -> Result<Client, ConnectionError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let has_peer_sharing = self.channels.has(protocol_numbers::PEER_SHARING);
        let mut handshake = self
            .channels
            .add_initiator()
            .map(HandshakeN2NClient::new)
            .unwrap();
        let handle = Handle::create(read_stream, write_stream, self.channels);
        let diffusion = handshake_n2n::DiffusionMode::InitiatorOnly;
        let peer_sharing = if has_peer_sharing {
            handshake_n2n::PeerSharing::Enabled
        } else {
            handshake_n2n::PeerSharing::Disabled
        };
        handshake
            .handshake(version, magic, diffusion, peer_sharing)
            .await?;
        Ok(Client { handle })
    }

    pub(crate) async fn build_n2c<R, W>(
        mut self,
        read_stream: R,
        write_stream: W,
        version: handshake_n2c::Version,
        magic: handshake_n2n::Magic,
    ) -> Result<Client, ConnectionError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let mut handshake = self
            .channels
            .add_initiator()
            .map(HandshakeN2CClient::new)
            .unwrap();
        let handle = Handle::create(read_stream, write_stream, self.channels);
        handshake.handshake(version, magic).await?;
        Ok(Client { handle })
    }
}
