//! connect to a cardano wallet extension in the browser
//!

use network_cardano::{ChainSyncClient, Client, ClientBuilder, Magic, Tip, VersionN2N};
use yew::{platform::spawn_local, prelude::*};

pub struct CardanoNetwork {
    chainsync: Option<ChainSyncClient>,

    handler: Option<Client>,
    error: Option<String>,
    tip: Option<Tip>,
}

#[derive(Properties, PartialEq)]
pub struct CardanoNetworkProperties {
    pub url: String,
}

pub enum CardanoNetworkMessage {
    Connected(Client),
    ConnectionFailed(String),
    Tip(ChainSyncClient, Tip),
}

impl Component for CardanoNetwork {
    type Message = CardanoNetworkMessage;

    type Properties = CardanoNetworkProperties;

    fn create(ctx: &Context<Self>) -> Self {
        let url = ctx.props().url.clone();
        let link = ctx.link().clone();

        let mut builder = ClientBuilder::new();
        let chainsync = builder.with_n2n_chainsync().unwrap();

        spawn_local(async move {
            match builder
                .ws_connect(url, VersionN2N::V14, Magic::CARDANO_MAINNET)
                .await
            {
                Ok(handler) => link.send_message(CardanoNetworkMessage::Connected(handler)),
                Err(error) => link.send_message(CardanoNetworkMessage::ConnectionFailed(format!(
                    "{error:#?}"
                ))),
            }
        });

        CardanoNetwork {
            chainsync: Some(chainsync),
            handler: None,
            error: None,
            tip: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            CardanoNetworkMessage::Connected(client) => {
                self.handler = Some(client);

                if let Some(mut chainsync) = self.chainsync.take() {
                    let link = ctx.link().clone();

                    spawn_local(async move {
                        match chainsync.get_tip().await {
                            Ok(tip) => {
                                link.send_message(CardanoNetworkMessage::Tip(chainsync, tip))
                            }
                            Err(error) => link.send_message(
                                CardanoNetworkMessage::ConnectionFailed(format!("{error:#?}")),
                            ),
                        }
                    });
                }

                true
            }
            CardanoNetworkMessage::ConnectionFailed(error) => {
                self.error = Some(error);
                true
            }
            CardanoNetworkMessage::Tip(chainsync, tip) => {
                self.chainsync = Some(chainsync);
                self.tip = Some(tip);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let error = if let Some(error) = self.error.as_ref() {
            html! {
                <div class="alert alert-danger">
                    {"Connection Error: "}{error}
                </div>
            }
        } else {
            html! {}
        };

        let connected = if let Some(..) = self.handler.as_ref() {
            html! {
            <div class="alert alert-success">
                {"Successfully connected!"}
            </div>
            }
        } else {
            html! {
                <div class="alert alert-info">
                    {"Attempting to connect..."}
                </div>
            }
        };

        let tip = if let Some(tip) = self.tip.as_ref() {
            let Tip {
                point,
                block_number,
            } = tip;
            html! {
            <div class="alert alert-success">
                {"block number: "} {block_number}
            </div>
            }
        } else {
            html! {}
        };

        html! {
          <div class="container">
            <div class="card mt-4">
                <div class="card-body">
                    <h5 class="card-title">{"Network Connection Status"}</h5>
                    <p class="card-text">
                        {"Connecting to: "}
                        <pre>{&ctx.props().url}</pre>
                    </p>
                    {error}
                    {connected}
                    {tip}
                </div>
            </div>
          </div>
        }
    }
    //
}
