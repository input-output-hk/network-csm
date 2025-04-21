use std::net::SocketAddr;

use clap::Parser;
use network_cardano::{ClientBuilder, Magic, VersionN2N};
use network_csm_cardano_protocols::blockfetch::Point;

#[derive(Debug, Parser)]
struct Arguments {
    #[arg(default_value = "147.75.92.75:3001")]
    address: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let Arguments { address } = Arguments::parse();

    let mut builder = ClientBuilder::new();
    let mut chainsync = builder.with_n2n_chainsync()?;
    let mut blockfetch = builder.with_blockfetch()?;

    let _client = builder
        .tcp_connect(address, VersionN2N::V14, Magic::CARDANO_MAINNET)
        .await?;

    let tip = chainsync.get_tip().await?;

    println!("{tip:?}");

    //let next = chainsync.request_next().await?;
    //println!("{next:?}");

    let end = Point::BlockHeader {
        slot_nb: 153527805,
        hash: [
            0xb6, 0xe2, 0xae, 0xd8, 0x89, 0x8d, 0x15, 0x9e, 0x88, 0xa1, 0x18, 0x47, 0x9f, 0xce,
            0x77, 0xc4, 0x39, 0x10, 0x28, 0xd7, 0xf4, 0x94, 0x56, 0xe1, 0x17, 0x25, 0xf0, 0xd3,
            0xf4, 0x90, 0x9e, 0x12,
        ],
    };
    let start = Point::BlockHeader {
        slot_nb: 153527771,
        hash: [
            0xb0, 0xe2, 0xdd, 0x4e, 0x23, 0x33, 0xbf, 0xfd, 0x25, 0x69, 0x6c, 0xc1, 0x41, 0xd9,
            0x6c, 0x50, 0x3e, 0x48, 0x95, 0xb3, 0x9e, 0x0b, 0x3a, 0x43, 0x6e, 0xd7, 0x6c, 0xd5,
            0x83, 0x51, 0x39, 0x21,
        ],
    };

    let mut count = 0;
    match blockfetch.request_range(start, end).await? {
        Some(mut fetcher) => {
            println!("fetching blocks");
            while let Some((_data, next_fetcher)) = fetcher.next().await? {
                println!("block received {}", count);
                tracing::info!("receive block data {}", count + 1);
                fetcher = next_fetcher;
                count += 1;
            }
        }
        None => (),
    }
    Ok(())
}
