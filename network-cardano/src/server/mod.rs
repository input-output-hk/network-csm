use network_csm::DuplicateChannel;
use network_csm_cardano_protocols::{handshake_n2c, handshake_n2n};
use network_csm_tokio::{Handle, HandleChannels};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    blockfetch::BlockFetchServer,
    chainsync::ChainSyncServer,
    handshake::{self, HandshakeN2CServer, HandshakeN2NServer},
};

#[cfg(all(not(target_arch = "wasm32")))]
pub mod socket;

pub struct ServerBuilder {
    channels: HandleChannels,
}

pub struct Server {
    #[allow(unused)]
    handle: Handle,
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Failed to establish secure handshake with peer")]
    Handshake(#[from] handshake::ServerError),
}

impl ServerBuilder {
    pub fn new() -> Self {
        let channels = HandleChannels::new();
        Self { channels }
    }

    pub fn with_n2n_chainsync(&mut self) -> Result<ChainSyncServer, DuplicateChannel> {
        self.channels.add_responder().map(ChainSyncServer::new_n2n)
    }

    pub fn with_n2c_chainsync(&mut self) -> Result<ChainSyncServer, DuplicateChannel> {
        self.channels.add_responder().map(ChainSyncServer::new_n2n)
    }

    pub fn with_blockfetch(&mut self) -> Result<BlockFetchServer, DuplicateChannel> {
        self.channels.add_responder().map(BlockFetchServer::new)
    }

    async fn accept_handshake_n2n<R, W, F>(
        mut self,
        read_stream: R,
        write_stream: W,
        f: F,
    ) -> Result<Server, ServerError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
        F: FnOnce(handshake_n2n::VersionProposal) -> handshake_n2n::ProposeVersionsRet,
    {
        let mut handshake = self
            .channels
            .add_responder()
            .map(HandshakeN2NServer::new)
            .unwrap();

        let handle = Handle::create(read_stream, write_stream, self.channels);
        handshake.handshake(f).await?;
        Ok(Server { handle })
    }

    pub(crate) async fn accept_handshake_n2c<R, W, F>(
        mut self,
        read_stream: R,
        write_stream: W,
        f: F,
    ) -> Result<Server, ServerError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
        F: FnOnce(handshake_n2c::VersionProposal) -> handshake_n2c::ProposeVersionsRet,
    {
        let mut handshake = self
            .channels
            .add_responder()
            .map(HandshakeN2CServer::new)
            .unwrap();

        let handle = Handle::create(read_stream, write_stream, self.channels);
        handshake.handshake(f).await?;
        Ok(Server { handle })
    }
}
