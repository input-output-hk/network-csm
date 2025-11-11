use network_csm_cardano_protocols::{handshake_n2c, handshake_n2n};
use network_csm_tokio::{AsyncChannel, MessageError};
use thiserror::Error;
use tracing_futures::Instrument;

pub struct HandshakeN2NClient(AsyncChannel<handshake_n2n::State>);

pub struct HandshakeN2CClient(AsyncChannel<handshake_n2c::State>);

pub struct HandshakeN2NServer(AsyncChannel<handshake_n2n::State>);

pub struct HandshakeN2CServer(AsyncChannel<handshake_n2c::State>);

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid Handshake reply: {0:?}")]
    N2NHandshakeReplyError(MessageError<handshake_n2n::State>),
    #[error("Connection refused: {0:?}")]
    N2NConnectionRefused(handshake_n2n::RefuseReason),

    #[error("Invalid Handshake reply: {0:?}")]
    N2CHandshakeReplyError(MessageError<handshake_n2c::State>),
    #[error("Connection refused: {0:?}")]
    N2CConnectionRefused(handshake_n2c::RefuseReason),
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Invalid Handshake query: {0:?}")]
    N2NHandshakeQueryError(MessageError<handshake_n2n::State>),

    #[error("Invalid Handshake query: {0:?}")]
    N2CHandshakeQueryError(MessageError<handshake_n2c::State>),
}

impl HandshakeN2NClient {
    pub fn new(channel: AsyncChannel<handshake_n2n::State>) -> Self {
        Self(channel)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn handshake(
        &mut self,
        version: handshake_n2n::Version,
        magic: handshake_n2n::Magic,
        diffusion: handshake_n2n::DiffusionMode,
        peer_sharing: handshake_n2n::PeerSharing,
    ) -> Result<(), Error> {
        tracing::trace!("initialising handshake");

        handshake_n2n(
            &mut self.0,
            version,
            handshake_n2n::HandshakeNodeData {
                magic,
                diffusion,
                peer_sharing,
                query: false,
            },
        )
        .in_current_span()
        .await
    }
}

impl HandshakeN2CClient {
    pub fn new(channel: AsyncChannel<handshake_n2c::State>) -> Self {
        Self(channel)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn handshake(
        &mut self,
        version: handshake_n2c::Version,
        magic: handshake_n2c::Magic,
    ) -> Result<(), Error> {
        tracing::trace!("initialising handshake");
        handshake_n2c(
            &mut self.0,
            version,
            handshake_n2c::HandshakeNodeData {
                magic,
                query: false,
            },
        )
        .in_current_span()
        .await
    }
}

#[tracing::instrument(
    skip(channel, data),
    fields(
        ?data.magic,
        ?data.diffusion,
        ?data.peer_sharing,
        data.query,
    ),
    err
)]
async fn handshake_n2n(
    channel: &mut AsyncChannel<handshake_n2n::State>,
    version: handshake_n2n::Version,
    data: handshake_n2n::HandshakeNodeData,
) -> Result<(), Error> {
    let versions_proposal = handshake_n2n::VersionProposal(vec![(version, data)]);

    tracing::trace!("submitting version proposal message");
    channel
        .write_one(handshake_n2n::Message::ProposeVersions(versions_proposal))
        .in_current_span()
        .await;
    let msg = channel
        .read_one_match(handshake_n2n::client_propose_versions_ret)
        .in_current_span()
        .await
        .map_err(Error::N2NHandshakeReplyError)?;
    match msg {
        handshake_n2n::ProposeVersionsRet::AcceptVersion(version, handshake_node_data) => {
            tracing::debug!("accepted {:?} {:?}", version, handshake_node_data);
            Ok(())
        }
        handshake_n2n::ProposeVersionsRet::Refuse(refuse_reason) => {
            Err(Error::N2NConnectionRefused(refuse_reason))
        }
        handshake_n2n::ProposeVersionsRet::QueryReply(version_proposal) => {
            // unsupported negotiation phase
            panic!("handshake query reply: {:?}", version_proposal)
        }
    }
}

#[tracing::instrument(
    skip(channel, data),
    fields(
        ?data.magic,
        data.query,
    ),
    err
)]
async fn handshake_n2c(
    channel: &mut AsyncChannel<handshake_n2c::State>,
    version: handshake_n2c::Version,
    data: handshake_n2c::HandshakeNodeData,
) -> Result<(), Error> {
    let versions_proposal = handshake_n2c::VersionProposal(vec![(version, data)]);

    channel
        .write_one(handshake_n2c::Message::ProposeVersions(versions_proposal))
        .in_current_span()
        .await;
    let msg = channel
        .read_one_match(handshake_n2c::client_propose_versions_ret)
        .in_current_span()
        .await
        .map_err(Error::N2CHandshakeReplyError)?;
    match msg {
        handshake_n2c::ProposeVersionsRet::AcceptVersion(version, handshake_node_data) => {
            tracing::debug!("accepted {:?} {:?}", version, handshake_node_data);
            Ok(())
        }
        handshake_n2c::ProposeVersionsRet::Refuse(refuse_reason) => {
            Err(Error::N2CConnectionRefused(refuse_reason))
        }
        handshake_n2c::ProposeVersionsRet::QueryReply(version_proposal) => {
            // unsupported negotiation phase
            panic!("handshake query reply: {:?}", version_proposal)
        }
    }
}

impl HandshakeN2NServer {
    pub fn new(channel: AsyncChannel<handshake_n2n::State>) -> Self {
        Self(channel)
    }

    #[tracing::instrument(skip(self, f), err)]
    pub async fn handshake<F>(&mut self, f: F) -> Result<(), ServerError>
    where
        F: FnOnce(handshake_n2n::VersionProposal) -> handshake_n2n::ProposeVersionsRet,
    {
        tracing::trace!("initialising handshake");
        let version_proposal = self
            .0
            .read_one_match(handshake_n2n::server_propose_message_filter)
            .await
            .map_err(ServerError::N2NHandshakeQueryError)?;

        let ret = f(version_proposal);

        self.0.write_one(handshake_n2n::Message::from(ret)).await;
        Ok(())
    }
}

impl HandshakeN2CServer {
    pub fn new(channel: AsyncChannel<handshake_n2c::State>) -> Self {
        Self(channel)
    }

    #[tracing::instrument(skip(self, f), err)]
    pub async fn handshake<F>(&mut self, f: F) -> Result<(), ServerError>
    where
        F: FnOnce(handshake_n2c::VersionProposal) -> handshake_n2c::ProposeVersionsRet,
    {
        tracing::trace!("initialising handshake");
        let version_proposal = self
            .0
            .read_one_match(handshake_n2c::server_propose_message_filter)
            .await
            .map_err(ServerError::N2CHandshakeQueryError)?;

        let ret = f(version_proposal);

        self.0.write_one(handshake_n2c::Message::from(ret)).await;
        Ok(())
    }
}
