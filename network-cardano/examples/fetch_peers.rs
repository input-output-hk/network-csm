use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Result;
use tokio::net::TcpStream;

use network_csm_tokio::{HandleChannels, Handle, AsyncChannel};
use network_csm_cardano_protocols::{handshake_n2n, peer_sharing};

fn ipv4_to_string(ip_u32: u32, port: u16) -> String {
    let b1 = ((ip_u32 >> 24) & 0xFF) as u8;
    let b2 = ((ip_u32 >> 16) & 0xFF) as u8;
    let b3 = ((ip_u32 >> 8) & 0xFF) as u8;
    let b4 = (ip_u32 & 0xFF) as u8;
    format!("{}.{}.{}.{}:{}", b1, b2, b3, b4, port)
}

fn ipv6_to_string(w1: u32, w2: u32, w3: u32, w4: u32, port: u16) -> String {
    let segs = [
        (w1 >> 16) as u16, (w1 & 0xFFFF) as u16,
        (w2 >> 16) as u16, (w2 & 0xFFFF) as u16,
        (w3 >> 16) as u16, (w3 & 0xFFFF) as u16,
        (w4 >> 16) as u16, (w4 & 0xFFFF) as u16,
    ];
    let core = segs.iter().map(|s| format!("{:x}", s)).collect::<Vec<_>>().join(":");
    format!("[{}]:{}", core, port)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

   let target = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "relays-new.cardano-mainnet.iohk.io:3001".into());

let mut addrs = tokio::net::lookup_host(&target).await
    .map_err(|e| anyhow::anyhow!("DNS lookup failed for {target}: {e}"))?;
let addr: SocketAddr = addrs
    .next()
    .ok_or_else(|| anyhow::anyhow!("no addresses resolved for {target}"))?;

let magic_num: u32 = std::env::args()
    .nth(2)
    .unwrap_or_else(|| "1".into()) // preprod=1; mainnet=764824073
    .parse()
    .expect("magic must be integer");

let magic = handshake_n2n::Magic(magic_num as u64);


    // 1) Resolve and connect to the target
    let stream = TcpStream::connect(addr).await?;
    let (r, w) = stream.into_split();

    // 2) Setting up channels for N2N handshake and peerSharing
    let mut chans = HandleChannels::new();
    let mut hs: AsyncChannel<handshake_n2n::State> = chans.add_initiator().expect("handshake channel");
    let mut ps: AsyncChannel<peer_sharing::State> = chans.add_initiator().expect("peer_sharing channel");

    // 3) Start mux/demux
    let _handle = Handle::create(r, w, chans);

    // 4) Perform n2n handshake; ensure peer sharing is enabled
    let version = handshake_n2n::Version::V14;
    let data = handshake_n2n::HandshakeNodeData {
        magic,
        diffusion: handshake_n2n::DiffusionMode::InitiatorOnly,
        peer_sharing: handshake_n2n::PeerSharing::Enabled,
        query: false,
    };

    // Send version proposal and await accept
    {
        let versions_proposal = handshake_n2n::VersionProposal(vec![(version, data.clone())]);
        hs.write_one(handshake_n2n::Message::ProposeVersions(versions_proposal)).await;
        let _accepted = hs.read_one_match(handshake_n2n::client_propose_versions_ret).await
            .map_err(|e| anyhow::anyhow!("handshake failed: {:?}", e))?;
    }

    println!("✅ Handshake OK. Requesting peers...");

    // 5) Request peers (sharerequests, readsharepeers and print unique addresses)
    let mut seen = HashSet::<String>::new();

    for _ in 0..3 {
       
        ps.write_one(peer_sharing::Message::ShareRequest(32)).await;

        match ps.read_one().await {
            Ok(peer_sharing::Message::SharePeers(list)) => {
                for p in list {
                    let s = match p {
                        peer_sharing::Peer::IPV4(ip, port) => ipv4_to_string(ip, port),
                        peer_sharing::Peer::IPV6(w1,w2,w3,w4,port) => ipv6_to_string(w1,w2,w3,w4,port),
                    };
                    if seen.insert(s.clone()) {
                        println!("Found peer: {s}");
                    }
                }
            }
            Ok(peer_sharing::Message::Done) => break,
            Ok(peer_sharing::Message::ShareRequest(_)) => { /* ignore; server shouldn’t send */ }
            Err(e) => {
                eprintln!("peer sharing read error: {:?}", e);
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
// optional: tell the server we're done with PeerSharing
let _ = ps.write_one(peer_sharing::Message::Done).await;

// final summary
println!("Total unique peers: {}", seen.len());

    Ok(())
}
