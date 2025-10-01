use std::{collections::HashSet, net::SocketAddr, time::Duration};

use anyhow::Result;
use tokio::{
    net::{TcpListener, TcpStream},
    time::sleep,
};
use axum::{
    Router,
    routing::get,
    response::IntoResponse,
    extract::ws::{WebSocketUpgrade, WebSocket, Message},
};

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
    let core = segs
        .iter()
        .map(|s| format!("{:x}", s))
        .collect::<Vec<_>>()
        .join(":");
    format!("[{}]:{}", core, port)
}

/// WS upgrade handler.
async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws)
}

/// WS connection task.
async fn handle_ws(mut socket: WebSocket) {
    if let Err(e) = discover_and_stream(&mut socket).await {
        let _ = socket.send(Message::Text(format!("❌ Error: {e}"))).await;
    }
}

/// Connects to a Cardano node, performs N2N handshake, requests peers, and streams them over WS.
async fn discover_and_stream(socket: &mut WebSocket) -> Result<()> {
    // Preprod bootstrap + magic; swap to mainnet by changing both.
    let target = "preprod-node.world.dev.cardano.org:3001";
    let magic_num = 1u64; // mainnet = 764824073

    // Resolve and connect
    let mut addrs = tokio::net::lookup_host(target).await?;
    let addr: SocketAddr = addrs
        .next()
        .ok_or_else(|| anyhow::anyhow!("no addresses resolved for {target}"))?;

    let stream = TcpStream::connect(addr).await?;
    let (r, w) = stream.into_split();

    // Channels: handshake (n2n) + peer_sharing
    let mut chans = HandleChannels::new();
    let mut hs: AsyncChannel<handshake_n2n::State> =
        chans.add_initiator().expect("handshake channel");
    let mut ps: AsyncChannel<peer_sharing::State> =
        chans.add_initiator().expect("peer_sharing channel");

    // Start mux/demux
    let _handle = Handle::create(r, w, chans);

    // Handshake with PeerSharing enabled
    let version = handshake_n2n::Version::V14;
    let data = handshake_n2n::HandshakeNodeData {
        magic: handshake_n2n::Magic(magic_num),
        diffusion: handshake_n2n::DiffusionMode::InitiatorOnly,
        peer_sharing: handshake_n2n::PeerSharing::Enabled,
        query: false,
    };

    let versions_proposal = handshake_n2n::VersionProposal(vec![(version, data.clone())]);
    hs.write_one(handshake_n2n::Message::ProposeVersions(versions_proposal))
        .await;
    hs.read_one_match(handshake_n2n::client_propose_versions_ret)
        .await
        .map_err(|e| anyhow::anyhow!("handshake failed: {e:?}"))?;

    socket
        .send(Message::Text("✅ Handshake OK. Fetching peers...".into()))
        .await?;

    // Request peers a few times and stream unique addrs
    let mut seen = HashSet::<String>::new();

    for _ in 0..3 {
        ps.write_one(peer_sharing::Message::ShareRequest(32)).await;

        match ps.read_one().await {
            Ok(peer_sharing::Message::SharePeers(list)) => {
                for p in list {
                    let s = match p {
                        peer_sharing::Peer::IPV4(ip, port) => ipv4_to_string(ip, port),
                        peer_sharing::Peer::IPV6(w1, w2, w3, w4, port) => {
                            ipv6_to_string(w1, w2, w3, w4, port)
                        }
                    };
                    if seen.insert(s.clone()) {
                        socket
                            .send(Message::Text(format!("Found peer: {s}")))
                            .await?;
                    }
                }
            }
            Ok(peer_sharing::Message::Done) => break,
            Ok(_) => {}
            Err(e) => {
                socket
                    .send(Message::Text(format!("Error reading peer list: {e:?}")))
                    .await?;
                break;
            }
        }

        sleep(Duration::from_millis(500)).await;
    }

    socket
        .send(Message::Text(format!("✅ Total unique peers: {}", seen.len())))
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/ws", get(ws_handler));

    println!("🌐 WebSocket server running at ws://localhost:8080/ws");
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;

    Ok(())
}


