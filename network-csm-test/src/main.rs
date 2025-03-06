use fakepipe::mempipe;
use network_csm::Direction;
use network_csm_cardano_protocols::{
    chainsync_n2n::{self, CborChainsyncData},
    handshake_n2n,
};
use network_csm_tokio::{AsyncChannel, Handle, HandleChannels};
use tokio::io::{AsyncRead, AsyncWrite};

mod fakepipe;

pub struct ClientChannels {
    handshake: AsyncChannel<handshake_n2n::State>,
    chainsync: AsyncChannel<chainsync_n2n::State>,
}

pub fn setup_handle<R, W>(reader: R, writer: W, direction: Direction) -> (ClientChannels, Handle)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut channels = HandleChannels::new();

    let (handshake, chainsync) = match direction {
        Direction::Initiator => {
            let handshake = channels.add_initiator::<handshake_n2n::State>().unwrap();
            let chainsync = channels.add_initiator::<chainsync_n2n::State>().unwrap();
            (handshake, chainsync)
        }
        Direction::Responder => {
            let handshake = channels.add_responder::<handshake_n2n::State>().unwrap();
            let chainsync = channels.add_responder::<chainsync_n2n::State>().unwrap();
            (handshake, chainsync)
        }
    };

    let clients = ClientChannels {
        handshake,
        chainsync,
    };
    let handle = Handle::create(reader, writer, channels);
    (clients, handle)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let versions_proposal = handshake_n2n::VersionProposal(vec![(
        handshake_n2n::Version::V14,
        handshake_n2n::HandshakeNodeData {
            magic: handshake_n2n::Magic(764824073),
            diffusion: handshake_n2n::DiffusionMode::InitiatorOnly,
            peer_sharing: handshake_n2n::PeerSharing::Enabled,
            query: false,
        },
    )]);

    tracing_subscriber::fmt::init();

    let (handle_a, handle_b) = mempipe();

    let (client_channels, _handle_client) =
        setup_handle(handle_a.clone(), handle_a, Direction::Initiator);
    let (server_channels, _handle_server) =
        setup_handle(handle_b.clone(), handle_b, Direction::Responder);

    let client_task = tokio::spawn(async move {
        let ClientChannels {
            mut handshake,
            mut chainsync,
        } = client_channels;
        //
        handshake
            .write_one(handshake_n2n::Message::ProposeVersions(versions_proposal))
            .await;

        let msg = handshake
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

        for i in 0..10 {
            match i % 3 {
                0 => {
                    chainsync
                        .write_one(chainsync_n2n::Message::FindIntersect(
                            chainsync_n2n::Points(vec![]),
                        ))
                        .await;

                    match chainsync
                        .read_one_match(chainsync_n2n::client_find_intersect_ret)
                        .await
                        .unwrap()
                    {
                        chainsync_n2n::FindIntersectRet::IntersectionFound(_point, _tip) => {}
                        chainsync_n2n::FindIntersectRet::IntersectionNotFound(_tip) => {}
                    }
                }
                1 => {
                    chainsync
                        .write_one(chainsync_n2n::Message::RequestNext)
                        .await;

                    loop {
                        match chainsync
                            .read_one_match(chainsync_n2n::client_request_next_ret)
                            .await
                            .unwrap()
                        {
                            chainsync_n2n::RequestNextRet::AwaitReply => {
                                continue;
                            }
                            chainsync_n2n::RequestNextRet::RollForward(
                                _cbor_chainsync_data,
                                _tip,
                            ) => {
                                println!("roll forward");
                                break;
                            }
                            chainsync_n2n::RequestNextRet::RollBackward(_point, _tip) => {
                                println!("roll backward");
                                break;
                            }
                        }
                    }
                }
                _ => {
                    chainsync.write_one(chainsync_n2n::Message::SyncDone).await;
                    chainsync.replace_state(chainsync_n2n::State::Idle);
                }
            }
        }
    });

    let server_task = tokio::spawn(async move {
        let ClientChannels {
            mut handshake,
            mut chainsync,
        } = server_channels;
        let version_proposal = handshake
            .read_one_match(handshake_n2n::server_propose_message_filter)
            .await
            .unwrap();
        tracing::info!("client proposed {:?}", version_proposal);

        let v = version_proposal.0;
        handshake
            .write_one(handshake_n2n::Message::AcceptVersion(
                v[0].0,
                v[0].1.clone(),
            ))
            .await;

        let w = tokio::task::spawn(async move {
            loop {
                let c = chainsync
                    .read_one_match(chainsync_n2n::server_idle_message_filter)
                    .await
                    .unwrap();
                match c {
                    chainsync_n2n::OnIdleMsg::RequestNext => {
                        chainsync
                            .write_one(chainsync_n2n::Message::RollForward(
                                CborChainsyncData(vec![1, 2, 3]),
                                chainsync_n2n::Tip::ORIGIN,
                            ))
                            .await;
                    }
                    chainsync_n2n::OnIdleMsg::FindIntersect(_points) => {
                        chainsync
                            .write_one(chainsync_n2n::Message::IntersectionFound(
                                chainsync_n2n::Point::Origin,
                                chainsync_n2n::Tip::ORIGIN,
                            ))
                            .await;
                    }
                    chainsync_n2n::OnIdleMsg::SyncDone => {
                        // TODO this is reset the state so that chainsync protocol can still be used, the "spec" is useless on what this need to happens
                        chainsync.replace_state(chainsync_n2n::State::Idle);
                        ()
                    }
                }
            }
        });
        tokio::join!(w)
    });

    let (r1, r2) = tokio::join!(client_task, server_task);

    println!("client return: {:?}", r1);
    println!("server return: {:?}", r2);

    Ok(())
}
