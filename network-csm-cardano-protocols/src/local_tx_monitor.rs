use cbored::CborRepr;
use network_csm::{Direction, Id, Protocol};
use network_csm_macro::NetworkCsmStateTransition;

use alloc::format;

use crate::{
    protocol_numbers,
    tx_submission::{Tx, TxId},
};

impl Protocol for State {
    const PROTOCOL_NUMBER: Id = protocol_numbers::LOCAL_TX_MONITOR;
    const MESSAGE_MAX_SIZE: usize = 8192;

    type Message = Message;

    fn transition(self, message: &Self::Message) -> Option<Self> {
        message.can_transition(self)
    }
    fn direction(self) -> Option<Direction> {
        match self {
            State::Idle => Some(Direction::Initiator),
            State::Acquiring => Some(Direction::Responder),
            State::Acquired => Some(Direction::Initiator),
            State::BusyHasTx => Some(Direction::Responder),
            State::BusyNextTx => Some(Direction::Responder),
            State::BusyGetSizes => Some(Direction::Responder),
            State::BusyGetMeasures => Some(Direction::Responder),
            State::Done => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum State {
    #[default]
    Idle,
    Acquiring,
    Acquired,
    BusyNextTx,
    BusyHasTx,
    BusyGetSizes,
    BusyGetMeasures,
    Done,
}

#[derive(Debug, Clone, CborRepr, NetworkCsmStateTransition)]
#[cborrepr(enumtype = "tagvariant", skipkey = 4)]
#[network_csm_state_transition(State,
    [
        Idle + Acquire = Acquiring,
        Acquiring + Acquired = Acquired,
        // Acquired + AwaitAcquire = Acquiring ?
        Acquired + Release = Idle,
        Acquired + NextTx = BusyNextTx,
        BusyNextTx + ReplyNextTx = Acquired,
        Acquired + HasTx = BusyHasTx,
        BusyHasTx + ReplyHasTx = Acquired,
        Acquired + GetSizes = BusyGetSizes,
        BusyGetSizes + ReplyGetSizes = Acquired,
        Acquired + GetMeasures = BusyGetMeasures,
        BusyGetMeasures + ReplyGetMeasures = Acquired,
        Idle + Done = Done,
    ]
)]
pub enum Message {
    #[network_csm_client]
    Done,
    Acquire,
    Acquired(u64),
    Release,
    NextTx,
    //ReplyNextTx(Option<Tx>), TODO add support for Option here
    ReplyNextTx(Tx),
    HasTx(TxId),
    ReplyHasTx(bool),
    GetSizes,
    ReplyGetSizes(Sizes),
    GetMeasures,
    ReplyGetMeasures(u32, Measures),
}

#[derive(Debug, Clone, CborRepr)]
#[cborrepr(structure = "array")]
pub struct Sizes {
    pub size1: u32,
    pub size2: u32,
    pub size3: u32,
}

#[derive(Debug, Clone)]
pub struct Measures(Vec<(String, (u64, u64))>);

impl cbored::Encode for Measures {
    fn encode(&self, writer: &mut cbored::Writer) {
        writer.map_build(
            cbored::StructureLength::Definite(cbored::state::HeaderValue::canonical(
                self.0.len() as u64
            )),
            |writer| {
                for (key, val) in self.0.iter() {
                    writer.encode(key);
                    writer.array_build(
                        cbored::StructureLength::Definite(cbored::state::HeaderValue::canonical(2)),
                        |writer| {
                            writer.encode(&val.0);
                            writer.encode(&val.1);
                        },
                    )
                }
            },
        )
    }
}

impl cbored::Decode for Measures {
    fn decode<'a>(reader: &mut cbored::Reader<'a>) -> Result<Self, cbored::DecodeError> {
        let map = reader
            .map()
            .map_err(cbored::DecodeErrorKind::ReaderError)
            .map_err(|e| e.context::<Self>())?;
        Ok(Self(
            map.iter()
                .map(|(mut k, mut v)| {
                    let key = k.decode_one()?;
                    let a = v
                        .array()
                        .map_err(cbored::DecodeErrorKind::ReaderError)
                        .map_err(|e| e.context::<Self>())?;
                    if v.is_finished() {
                        return Err(cbored::DecodeErrorKind::Custom(
                            "array not finished".to_string(),
                        )
                        .context::<Self>());
                    }
                    let v1 = a[0].decode()?;
                    let v2 = a[1].decode()?;
                    Ok((key, (v1, v2)))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
