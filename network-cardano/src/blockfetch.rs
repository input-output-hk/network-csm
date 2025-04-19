use network_csm_cardano_protocols::blockfetch::{self, CborBlockData};
use network_csm_tokio::{AsyncChannel, MessageError};
use tracing_futures::Instrument;

pub struct BlockFetchClient(AsyncChannel<blockfetch::State>);

pub struct Blocks<'a> {
    client: &'a mut BlockFetchClient,
}

impl BlockFetchClient {
    pub fn new(channel: AsyncChannel<blockfetch::State>) -> Self {
        BlockFetchClient(channel)
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
    ) -> Result<Option<Blocks<'a>>, MessageError<blockfetch::State>> {
        let msg = blockfetch::Message::RequestRange(start, end);
        self.write_one(msg).in_current_span().await;
        match self
            .read_one_match(blockfetch::client_request_range_ret)
            .in_current_span()
            .await?
        {
            blockfetch::RequestRangeRet::NoBlocks => Ok(None),
            blockfetch::RequestRangeRet::StartBatch => Ok(Some(Blocks { client: self })),
        }
    }
}

impl<'a> Blocks<'a> {
    pub async fn next(
        self,
    ) -> Result<Option<(CborBlockData, Self)>, MessageError<blockfetch::State>> {
        match self.client.0.read_one().in_current_span().await? {
            blockfetch::Message::Block(cbor_block_data) => Ok(Some((cbor_block_data, self))),
            blockfetch::Message::BatchDone => Ok(None),
            blockfetch::Message::RequestRange(_, _)
            | blockfetch::Message::ClientDone
            | blockfetch::Message::StartBatch
            | blockfetch::Message::NoBlocks => {
                panic!("invalid")
            }
        }
    }
}
