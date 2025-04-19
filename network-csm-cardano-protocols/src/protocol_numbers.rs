use network_csm::Id;

pub const HANDSHAKE: Id = Id::ZERO;
pub const CHAINSYNC_N2N: Id = Id::new(2);
pub const BLOCKFETCH: Id = Id::new(3);
pub const TX_SUBMISSION: Id = Id::new(4);
pub const CHAINSYNC_N2C: Id = Id::new(5);
pub const LOCAL_TX_SUBMISSION: Id = Id::new(6);
pub const LOCAL_STATE_QUERY: Id = Id::new(7);
pub const KEEP_ALIVE: Id = Id::new(8);
pub const LOCAL_TX_MONITOR: Id = Id::new(9);
pub const PEER_SHARING: Id = Id::new(10);
