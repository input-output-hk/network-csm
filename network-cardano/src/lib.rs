mod blockfetch;
mod chainsync;
pub mod client;
pub(crate) mod handshake;

pub type VersionN2N = network_csm_cardano_protocols::handshake_n2n::Version;
pub type VersionN2C = network_csm_cardano_protocols::handshake_n2c::Version;
pub type Magic = network_csm_cardano_protocols::handshake_n2n::Magic;

pub use self::{
    blockfetch::BlockFetchClient,
    chainsync::{ChainSyncClient, Tip},
    client::common::{Client, ClientBuilder},
};
