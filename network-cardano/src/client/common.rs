use crate::{ChainSyncClient, handshake::HandshakeClient};
use network_csm::DuplicateChannel;
use network_csm_tokio::{Handle, HandleChannels};
use tokio::io::{AsyncRead, AsyncWrite};

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

    pub fn with_n2n_handshake(&mut self) -> Result<HandshakeClient, DuplicateChannel> {
        self.channels.add_initiator().map(HandshakeClient::new_n2n)
    }

    pub fn with_n2c_handshake(&mut self) -> Result<HandshakeClient, DuplicateChannel> {
        self.channels.add_initiator().map(HandshakeClient::new_n2c)
    }

    pub fn with_n2n_chainsync(&mut self) -> Result<ChainSyncClient, DuplicateChannel> {
        self.channels.add_initiator().map(ChainSyncClient::new_n2n)
    }

    pub fn with_n2c_chainsync(&mut self) -> Result<ChainSyncClient, DuplicateChannel> {
        self.channels.add_initiator().map(ChainSyncClient::new_n2n)
    }

    pub fn build<R, W>(self, read_stream: R, write_stream: W) -> Client
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let handle = Handle::create(read_stream, write_stream, self.channels);

        Client { handle }
    }
}
