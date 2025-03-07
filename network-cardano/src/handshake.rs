use network_csm_cardano_protocols::{handshake_n2c, handshake_n2n};
use network_csm_tokio::{AsyncChannel, MessageError};
use thiserror::Error;
use tracing_futures::Instrument;

pub enum HandshakeClient {
    N2N(AsyncChannel<handshake_n2n::State>),
    N2C(AsyncChannel<handshake_n2c::State>),
}

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

impl HandshakeClient {
    pub fn new_n2n(channel: AsyncChannel<handshake_n2n::State>) -> Self {
        Self::N2N(channel)
    }

    pub fn new_n2c(channel: AsyncChannel<handshake_n2c::State>) -> Self {
        Self::N2C(channel)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn handshake(&mut self) -> Result<(), Error> {
        tracing::trace!("initialising handshake");

        match self {
            Self::N2N(n2n) => {
                handshake_n2n(
                    n2n,
                    handshake_n2n::Version::V14,
                    handshake_n2n::HandshakeNodeData {
                        magic: handshake_n2n::Magic(764824073),
                        diffusion: handshake_n2n::DiffusionMode::InitiatorOnly,
                        peer_sharing: handshake_n2n::PeerSharing::Enabled,
                        query: false,
                    },
                )
                .in_current_span()
                .await
            }
            Self::N2C(n2c) => {
                handshake_n2c(
                    n2c,
                    handshake_n2c::Version::V16,
                    handshake_n2c::HandshakeNodeData {
                        magic: handshake_n2c::Magic(764824073),
                        query: false,
                    },
                )
                .in_current_span()
                .await
            }
        }
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
    tracing::trace!("waiting server's reply");
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
