use network_csm_cardano_protocols::blockfetch::{self, CborBlockData};
use network_csm_tokio::{AsyncChannel, MessageError};
use tracing_futures::Instrument;

pub struct BlockFetchClient(AsyncChannel<blockfetch::State>);

pub struct BlockFetchServer(AsyncChannel<blockfetch::State>);

pub struct BlocksFetcher<'a> {
    client: &'a mut BlockFetchClient,
}

impl BlockFetchClient {
    pub fn new(channel: AsyncChannel<blockfetch::State>) -> Self {
        Self(channel)
    }

    #[tracing::instrument(skip(self))]
    async fn write_one(&mut self, msg: blockfetch::Message) {
        self.0.write_one(msg).in_current_span().await
    }

    #[tracing::instrument(skip(self, f))]
    async fn read_one_match<F, T>(&mut self, f: F) -> Result<T, MessageError<blockfetch::State>>
    where
        F: FnOnce(blockfetch::Message) -> Option<T>,
    {
        self.0.read_one_match(f).await
    }

    pub async fn request_range<'a>(
        &'a mut self,
        start: blockfetch::Point,
        end: blockfetch::Point,
    ) -> Result<Option<BlocksFetcher<'a>>, MessageError<blockfetch::State>> {
        let msg = blockfetch::Message::RequestRange(start, end);
        self.write_one(msg).in_current_span().await;
        match self
            .read_one_match(blockfetch::client_request_range_ret)
            .in_current_span()
            .await?
        {
            blockfetch::RequestRangeRet::NoBlocks => Ok(None),
            blockfetch::RequestRangeRet::StartBatch => Ok(Some(BlocksFetcher { client: self })),
        }
    }
}

impl<'a> BlocksFetcher<'a> {
    pub async fn next(
        self,
    ) -> Result<Option<(CborBlockData, Self)>, MessageError<blockfetch::State>> {
        match self.client.0.read_one().in_current_span().await? {
            blockfetch::Message::Block(cbor_block_data) => Ok(Some((cbor_block_data, self))),
            blockfetch::Message::BatchDone => Ok(None),
            // invalid messages in this context
            blockfetch::Message::RequestRange(_, _)
            | blockfetch::Message::ClientDone
            | blockfetch::Message::StartBatch
            | blockfetch::Message::NoBlocks => {
                panic!("invalid")
            }
        }
    }
}

impl BlockFetchServer {
    pub fn new(channel: AsyncChannel<blockfetch::State>) -> Self {
        Self(channel)
    }

    #[tracing::instrument(skip(self))]
    async fn write_one(&mut self, msg: blockfetch::Message) {
        self.0.write_one(msg).in_current_span().await
    }

    #[tracing::instrument(skip(self, f))]
    async fn read_one_match<F, T>(&mut self, f: F) -> Result<T, MessageError<blockfetch::State>>
    where
        F: FnOnce(blockfetch::Message) -> Option<T>,
    {
        self.0.read_one_match(f).await
    }

    // TODO in-progress API
    pub async fn idle<F, Fut, R>(
        &mut self,
        f: F,
    ) -> Result<Option<R>, MessageError<blockfetch::State>>
    where
        F: FnOnce(blockfetch::Point, blockfetch::Point) -> Fut,
        Fut: Future<Output = Option<R>>,
    {
        match self
            .0
            .read_one_match(blockfetch::server_idle_message_filter)
            .await?
        {
            blockfetch::OnIdleMsg::RequestRange(point_start, point_end) => {
                let r = f(point_start, point_end).await;
                let reply_msg = if r.is_some() {
                    blockfetch::Message::StartBatch
                } else {
                    blockfetch::Message::NoBlocks
                };
                self.0.write_one(reply_msg).await;
                Ok(r)
            }
            blockfetch::OnIdleMsg::ClientDone => Ok(None),
        }
    }
}
