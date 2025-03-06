use network_csm_cardano_protocols::{chainsync_n2c, chainsync_n2n, handshake_n2c, handshake_n2n};
use network_csm_tokio::{Handle, HandleChannels};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();

    tracing_subscriber::fmt::init();

    if args.len() > 1 {
        let path = &args[1];
        println!("unix connecting to {:?}", path);
        main_unix(&path).await
    } else {
        main_tcp_connect().await
    }
}

async fn main_unix(path: &str) -> anyhow::Result<()> {
    let mut channels = HandleChannels::new();
    let mut h = channels.add_initiator::<handshake_n2c::State>().unwrap();
    let mut _c = channels.add_initiator::<chainsync_n2c::State>().unwrap();

    let _handle = Handle::connect_unix(&path, channels).await?;

    let versions_proposal = handshake_n2c::VersionProposal(vec![(
        handshake_n2c::Version::V16,
        handshake_n2c::HandshakeNodeData {
            magic: handshake_n2c::Magic(764824073),
            query: false,
        },
    )]);

    h.write_one(handshake_n2c::Message::ProposeVersions(versions_proposal))
        .await;
    let msg = h
        .read_one_match(handshake_n2c::client_propose_versions_ret)
        .await
        .unwrap();
    match msg {
        handshake_n2c::ProposeVersionsRet::AcceptVersion(version, handshake_node_data) => {
            tracing::info!("accepted {:?} {:?}", version, handshake_node_data)
        }
        handshake_n2c::ProposeVersionsRet::Refuse(refuse_reason) => {
            panic!("handshake refused: {:?}", refuse_reason)
        }
        handshake_n2c::ProposeVersionsRet::QueryReply(version_proposal) => {
            panic!("handshake query reply: {:?}", version_proposal)
        }
    }
    Ok(())
}

async fn main_tcp_connect() -> anyhow::Result<()> {
    const BOOTSTRAP1: &str = "backbone.mainnet.cardanofoundation.org.";
    const BOOTSTRAP2: &str = "backbone.cardano.iog.io.";
    const BOOTSTRAP3: &str = "backbone.mainnet.emurgornd.com";
    const PORT: u16 = 3001;

    let bootstraps = vec![(BOOTSTRAP1, PORT), (BOOTSTRAP2, PORT), (BOOTSTRAP3, PORT)];

    let mut channels = HandleChannels::new();
    let mut h = channels.add_initiator::<handshake_n2n::State>().unwrap();
    let mut c = channels.add_initiator::<chainsync_n2n::State>().unwrap();

    let versions_proposal = handshake_n2n::VersionProposal(vec![(
        handshake_n2n::Version::V14,
        handshake_n2n::HandshakeNodeData {
            magic: handshake_n2n::Magic(764824073),
            diffusion: handshake_n2n::DiffusionMode::InitiatorOnly,
            peer_sharing: handshake_n2n::PeerSharing::Enabled,
            query: false,
        },
    )]);

    let _handle = Handle::connect_tcp(&bootstraps, channels).await?;

    h.write_one(handshake_n2n::Message::ProposeVersions(versions_proposal))
        .await;
    let msg = h
        .read_one_match(handshake_n2n::client_propose_versions_ret)
        .await
        .unwrap();
    match msg {
        handshake_n2n::ProposeVersionsRet::AcceptVersion(version, handshake_node_data) => {
            tracing::info!("accepted {:?} {:?}", version, handshake_node_data)
        }
        handshake_n2n::ProposeVersionsRet::Refuse(refuse_reason) => {
            panic!("handshake refused: {:?}", refuse_reason)
        }
        handshake_n2n::ProposeVersionsRet::QueryReply(version_proposal) => {
            panic!("handshake query reply: {:?}", version_proposal)
        }
    }

    for _ in 0..2 {
        let msg = chainsync_n2n::Message::FindIntersect(chainsync_n2n::Points(vec![
            chainsync_n2n::Point::Origin,
        ]));
        c.write_one(msg).await;
        let msg = c
            .read_one_match(chainsync_n2n::client_find_intersect_ret)
            .await
            .unwrap();
        match msg {
            chainsync_n2n::FindIntersectRet::IntersectionFound(point, tip) => {
                println!("find intersect found point={} tip={}", point, tip)
            }
            chainsync_n2n::FindIntersectRet::IntersectionNotFound(tip) => {
                println!("find intersect not found tip={}", tip)
            }
        }
        let msg = chainsync_n2n::Message::SyncDone;
        c.write_one(msg).await;
        let msg = chainsync_n2n::Message::SyncDone;
        c.write_one(msg).await;
        c.replace_state(chainsync_n2n::State::Idle);
    }

    let msg = chainsync_n2n::Message::SyncDone;
    c.write_one(msg).await;

    Ok(())
}
