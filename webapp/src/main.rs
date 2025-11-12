use ce_cardano_network_webapp::CardanoNetwork;
use yew::prelude::*;

#[function_component]
fn App() -> Html {
    html! {
        <>
        <div class="container mt-5">
            <div class="row">
                <div class="col-md-8 offset-md-2">
                <h1 class="display-4 text-center mb-4">{"Cardano Network Webapp"}</h1>
                <p class="lead text-center text-muted mb-5">{"A webapplication connecting to the cardano network and requesting the latest TIP"}</p>
                <div class="text-center mb-4">
                    <a href="https://github.com/input-output-hk/ce-cardano-network-webapp" target="_blank" class="text-dark">
                        <i class="fa fa-github fa-2x"></i>
                        <span class="ms-2">{"View source on GitHub"}</span>
                    </a>
                </div>
                </div>
            </div>
        </div>
        <div class="container mt-5">
            <div class="row">
                <div class="col-md-8 offset-md-2">

                  <CardanoNetwork url="ws://localhost.:3000/" />
                </div>
            </div>
        </div>
        </>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());

    yew::Renderer::<App>::new().render();
}
