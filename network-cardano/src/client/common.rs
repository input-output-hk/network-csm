use crate::{ChainSyncClient, handshake::HandshakeClient};
use network_csm::DuplicateChannel;
use network_csm_tokio::{Handle, HandleChannels};
use tokio::io::{AsyncRead, AsyncWrite};

use super::ConnectionError;

/// [`ClientBuilder`] to establish a client connection with a remote
/// peer.
///
pub struct ClientBuilder {
    channels: HandleChannels,
    expect_n2n: bool,
    expect_n2c: bool,
}

pub struct Client {
    #[allow(unused)]
    handle: Handle,
}

impl ClientBuilder {
    pub fn new() -> Self {
        let channels = HandleChannels::new();
        Self {
            channels,
            expect_n2n: false,
            expect_n2c: false,
        }
    }

    fn with_n2n_handshake(&mut self) -> Result<HandshakeClient, DuplicateChannel> {
        self.channels.add_initiator().map(HandshakeClient::new_n2n)
    }

    fn with_n2c_handshake(&mut self) -> Result<HandshakeClient, DuplicateChannel> {
        self.channels.add_initiator().map(HandshakeClient::new_n2c)
    }

    pub fn with_n2n_chainsync(&mut self) -> Result<ChainSyncClient, DuplicateChannel> {
        self.expect_n2n = true;
        self.channels.add_initiator().map(ChainSyncClient::new_n2n)
    }

    pub fn with_n2c_chainsync(&mut self) -> Result<ChainSyncClient, DuplicateChannel> {
        self.expect_n2c = true;
        self.channels.add_initiator().map(ChainSyncClient::new_n2n)
    }

    pub(crate) async fn build<R, W>(
        mut self,
        read_stream: R,
        write_stream: W,
    ) -> Result<Client, ConnectionError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let mut handshake = match (self.expect_n2n, self.expect_n2c) {
            (true, true) => return Err(ConnectionError::ProtocolConflict),
            (false, false) => return Err(ConnectionError::ProtocolNotSpecified),
            (true, false) => self.with_n2n_handshake().unwrap(),
            (false, true) => self.with_n2c_handshake().unwrap(),
        };

        let handle = Handle::create(read_stream, write_stream, self.channels);

        handshake.handshake().await?;

        Ok(Client { handle })
    }
}
