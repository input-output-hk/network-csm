use network_csm_cardano_protocols::chainsync_n2c;
use network_csm_cardano_protocols::chainsync_n2n::{self, CborChainsyncData, Point, Points};
use network_csm_tokio::{AsyncChannel, MessageError};
use tracing_futures::Instrument;

pub use chainsync_n2n::Tip;

pub enum ChainSyncClient {
    N2N(AsyncChannel<chainsync_n2n::State>),
    N2C(AsyncChannel<chainsync_n2c::State>),
}

#[derive(Debug, Clone)]
pub enum RequestNext {
    Forward(CborChainsyncData, Tip),
    Backward(Point, Tip),
}

impl ChainSyncClient {
    pub fn new_n2n(channel: AsyncChannel<chainsync_n2n::State>) -> ChainSyncClient {
        ChainSyncClient::N2N(channel)
    }

    pub fn new_n2c(channel: AsyncChannel<chainsync_n2c::State>) -> ChainSyncClient {
        ChainSyncClient::N2C(channel)
    }

    #[tracing::instrument(skip(self))]
    async fn write_one(&mut self, msg: chainsync_n2n::Message) {
        match self {
            ChainSyncClient::N2N(async_channel) => {
                async_channel.write_one(msg).in_current_span().await
            }
            ChainSyncClient::N2C(async_channel) => {
                async_channel.write_one(msg).in_current_span().await
            }
        }
    }

    #[tracing::instrument(skip(self, f))]
    async fn read_one_match<F, T>(&mut self, f: F) -> Result<T, MessageError<chainsync_n2n::State>>
    where
        F: FnOnce(chainsync_n2n::Message) -> Option<T>,
    {
        match self {
            ChainSyncClient::N2N(async_channel) => async_channel.read_one_match(f).await,
            ChainSyncClient::N2C(async_channel) => async_channel
                .read_one_match(f)
                .in_current_span()
                .await
                .map_err(|e| e.map_state(|st, msg| (st.into(), msg))),
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_tip(&mut self) -> Result<Tip, MessageError<chainsync_n2n::State>> {
        let msg = chainsync_n2n::Message::FindIntersect(Points(vec![Point::Origin]));
        self.write_one(msg).in_current_span().await;
        match self
            .read_one_match(chainsync_n2n::client_find_intersect_ret)
            .in_current_span()
            .await?
        {
            chainsync_n2n::FindIntersectRet::IntersectionFound(_point, tip) => Ok(tip),
            chainsync_n2n::FindIntersectRet::IntersectionNotFound(tip) => Ok(tip),
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn request_next(
        &mut self,
    ) -> Result<RequestNext, MessageError<chainsync_n2n::State>> {
        let msg = chainsync_n2n::Message::RequestNext;
        self.write_one(msg).in_current_span().await;

        loop {
            match self
                .read_one_match(chainsync_n2n::client_request_next_ret)
                .in_current_span()
                .await?
            {
                chainsync_n2n::RequestNextRet::AwaitReply => {}
                chainsync_n2n::RequestNextRet::RollForward(cbor_chainsync_data, tip) => {
                    return Ok(RequestNext::Forward(cbor_chainsync_data, tip));
                }
                chainsync_n2n::RequestNextRet::RollBackward(point, tip) => {
                    return Ok(RequestNext::Backward(point, tip));
                }
            }
        }
    }
}
