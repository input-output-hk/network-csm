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

    let start = Point::BlockHeader {
        slot_nb: 153100809,
        hash: [
            0xfd, 0x84, 0x5f, 0x2b, 0x84, 0xe7, 0xdd, 0xe8, 0x17, 0x6c, 0xbd, 0x98, 0xf8, 0x9f,
            0x93, 0xd1, 0x06, 0x8b, 0xcb, 0xfc, 0xef, 0x70, 0xc2, 0x7d, 0xa3, 0x7b, 0xd3, 0x14,
            0x8a, 0xab, 0x60, 0x30,
        ],
    };
    let end = tip.point;

    match blockfetch.request_range(start, end).await? {
        Some(mut fetcher) => {
            while let Some((data, next_fetcher)) = fetcher.next().await? {
                println!("receive data");
                fetcher = next_fetcher;
            }
        }
        None => (),
    }
    Ok(())
}
