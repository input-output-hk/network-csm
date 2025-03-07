//! connect to a cardano wallet extension in the browser
//!

use network_cardano::{ChainSyncClient, Client, ClientBuilder};
use yew::{platform::spawn_local, prelude::*};

pub struct CardanoNetwork {
    chainsync: ChainSyncClient,

    handler: Option<Client>,
    error: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct CardanoNetworkProperties {
    pub url: String,
}

pub enum CardanoNetworkMessage {
    Connected(Client),
    ConnectionFailed(String),
}

impl Component for CardanoNetwork {
    type Message = CardanoNetworkMessage;

    type Properties = CardanoNetworkProperties;

    fn create(ctx: &Context<Self>) -> Self {
        let url = ctx.props().url.clone();
        let link = ctx.link().clone();

        let mut builder = ClientBuilder::new();
        let chainsync = builder.with_n2c_chainsync().unwrap();

        spawn_local(async move {
            match builder.ws_connect(url).await {
                Ok(handler) => link.send_message(CardanoNetworkMessage::Connected(handler)),
                Err(error) => {
                    link.send_message(CardanoNetworkMessage::ConnectionFailed(error.to_string()))
                }
            }
        });

        CardanoNetwork {
            chainsync,
            handler: None,
            error: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            CardanoNetworkMessage::Connected(client) => {
                self.handler = Some(client);
                true
            }
            CardanoNetworkMessage::ConnectionFailed(error) => {
                self.error = Some(error);
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
                </div>
            </div>
          </div>
        }
    }
    //
}
