use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use network_csm::{
    Channel as RawChannel, ChannelsMap, ChannelsMapBuilder, Direction, DuplicateChannel, Id,
    OnDirection, Protocol, ReadMessageError,
};

pub struct Sending {
    position: usize,
    data: Vec<u8>,
}

impl Sending {
    pub fn new(data: Vec<u8>) -> Self {
        Self { position: 0, data }
    }

    pub fn left(&self) -> &[u8] {
        &self.data[self.position..]
    }

    pub fn advance(&mut self, n: usize) {
        self.position += n;
    }
}

#[derive(Clone)]
pub struct AsyncRawChannel {
    pub direction: Direction,

    /// Raw channel
    pub(crate) raw_channel: RawChannel,

    pub(crate) to_send: Arc<std::sync::Mutex<Option<Sending>>>,

    /// Raw channel
    pub(crate) terminated: Arc<AtomicBool>,
    /// Notification for writing has happened in channel
    pub(crate) w_notify: Arc<tokio::sync::Notify>,
    /// Notification for data has been added to read
    pub(crate) r_notify: Arc<tokio::sync::Notify>,
    /// Notification for sending has happened in channel
    pub(crate) sending_notify: Arc<tokio::sync::Notify>,
}

impl AsyncRawChannel {
    pub fn new<P: Protocol>(
        direction: Direction,
        message_max_size: usize,
        w_notify: Arc<tokio::sync::Notify>,
    ) -> Self {
        let r_notify = Arc::new(tokio::sync::Notify::new());
        let sending_notify = Arc::new(tokio::sync::Notify::new());
        Self {
            direction,
            raw_channel: RawChannel::new(message_max_size),
            to_send: Arc::new(std::sync::Mutex::new(None)),
            terminated: Arc::new(AtomicBool::new(false)),
            w_notify,
            r_notify,
            sending_notify,
        }
    }

    pub fn terminate(&self) {
        self.terminated.store(true, Ordering::SeqCst)
    }

    pub async fn send_one<P: Protocol>(&mut self, message: P::Message) {
        let mut writer = cbored::Writer::new();
        writer.encode(&message);
        let data = writer.finalize();

        loop {
            {
                let mut to_send = self.to_send.lock().unwrap();
                if to_send.is_none() {
                    *to_send = Some(Sending::new(data));
                    break;
                }
            }
            self.sending_notify.notified().await;
        }
        self.w_notify.notify_one();
    }

    async fn read_one<P: Protocol>(&mut self) -> Result<P::Message, MessageError<P>> {
        loop {
            match self.raw_channel.pop_message() {
                Some(m) => return m.map_err(|e| e.into()),
                None => {
                    if self.terminated.load(Ordering::SeqCst) == true {
                        return Err(MessageError::StreamTerminated);
                    }
                    self.r_notify.notified().await
                }
            }
        }
    }
}

pub struct AsyncChannel<P: Protocol> {
    pub(crate) channel: AsyncRawChannel,
    pub(crate) protocol: P,
}

#[derive(Clone, thiserror::Error, Debug)]
pub enum MessageError<P: Protocol> {
    #[error("Invalid content")]
    InvalidContent(#[source] ReadMessageError),
    #[error("Invalid state")]
    InvalidState { current: P, msg: P::Message },
    #[error("Stream terminated")]
    StreamTerminated,
    #[error("Internal error")]
    InternalError,
}

impl<P: Protocol> MessageError<P> {
    pub fn map_state<F, O: Protocol>(self, f: F) -> MessageError<O>
    where
        F: FnOnce(P, P::Message) -> (O, O::Message),
    {
        match self {
            MessageError::InvalidContent(read_message_error) => {
                MessageError::InvalidContent(read_message_error)
            }
            MessageError::InvalidState { current, msg } => {
                let (o, omsg) = f(current, msg);
                MessageError::InvalidState {
                    current: o,
                    msg: omsg,
                }
            }
            MessageError::StreamTerminated => MessageError::StreamTerminated,
            MessageError::InternalError => MessageError::InternalError,
        }
    }
}

impl<P: Protocol> From<ReadMessageError> for MessageError<P> {
    fn from(r: ReadMessageError) -> Self {
        MessageError::InvalidContent(r)
    }
}

impl<P: Protocol> AsyncChannel<P> {
    pub fn new(direction: Direction, protocol: P, mux_notify: Arc<tokio::sync::Notify>) -> Self {
        Self {
            channel: AsyncRawChannel::new::<P>(direction, P::MESSAGE_MAX_SIZE, mux_notify),
            protocol,
        }
    }

    /// Set the state of a protocol to a given value. this is not recommended to
    /// use in general, but this is exposed to build tools that don't want to
    /// deal with the normal, for example injecting bad packets for testing.
    #[doc(hidden)]
    pub fn replace_state(&mut self, protocol: P) {
        self.protocol = protocol
    }

    pub fn channel_id(&self) -> Id {
        P::PROTOCOL_NUMBER
    }

    pub fn get_state(&self) -> P {
        self.protocol
    }

    pub fn raw(&self) -> &AsyncRawChannel {
        &self.channel
    }

    /// Read a message from the channel and try to update the state
    /// from the current state to the new state with the new received message
    ///
    /// If the message received is not expected, then an error is return that contains
    /// the message and the current state of the protocol
    pub async fn read_one(&mut self) -> Result<P::Message, MessageError<P>> {
        let m = self.channel.read_one::<P>().await?;
        match self.protocol.transition(&m) {
            None => {
                return Err(MessageError::InvalidState {
                    current: self.protocol,
                    msg: m,
                });
            }
            Some(new_state) => {
                self.protocol = new_state;
                Ok(m)
            }
        }
    }

    /// Read a message from the channel and try to update the state and match it through a selecting function
    pub async fn read_one_match<F, T>(&mut self, msg_match: F) -> Result<T, MessageError<P>>
    where
        F: FnOnce(P::Message) -> Option<T>,
    {
        let m = self.channel.read_one::<P>().await?;
        match self.protocol.transition(&m) {
            None => {
                return Err(MessageError::InvalidState {
                    current: self.protocol,
                    msg: m,
                });
            }
            Some(new_state) => match msg_match(m) {
                None => {
                    tracing::error!(
                        "Network-CSM: Internal Error: state transition from {:?} succeeded to {:?} but matching function failed to capture the valid message",
                        self.protocol,
                        new_state
                    );
                    Err(MessageError::InternalError)
                }
                Some(t) => {
                    self.protocol = new_state;
                    Ok(t)
                }
            },
        }
    }

    pub async fn write_one(&mut self, message: P::Message) {
        match self.protocol.transition(&message) {
            None => {
                tracing::warn!("invalid message to send current-state={:?}", self.protocol)
            }
            Some(new_state) => {
                self.protocol = new_state;
            }
        }
        self.channel.send_one::<P>(message).await
    }
}

pub struct HandleChannels {
    pub(crate) mux_notify: Arc<tokio::sync::Notify>,
    channels: ChannelsMapBuilder<OnDirection<AsyncRawChannel>>,
}

impl HandleChannels {
    #[must_use]
    pub fn new() -> Self {
        let mux_notify = Arc::new(tokio::sync::Notify::new());
        let channels = ChannelsMapBuilder::new();
        Self {
            mux_notify,
            channels,
        }
    }

    #[must_use]
    pub fn add<P>(
        &mut self,
        direction: OnDirection<()>,
    ) -> Result<OnDirection<AsyncChannel<P>>, DuplicateChannel>
    where
        P: Protocol + Default,
    {
        Self::add_with(self, P::default(), direction)
    }

    #[must_use]
    pub fn add_initiator<P>(&mut self) -> Result<AsyncChannel<P>, DuplicateChannel>
    where
        P: Protocol + Default,
    {
        Self::add_with(self, P::default(), OnDirection::Initiator(())).map(|dir| match dir {
            OnDirection::Initiator(t) => t,
            _ => {
                panic!("internal error: should return only initiator value")
            }
        })
    }

    #[must_use]
    pub fn add_responder<P>(&mut self) -> Result<AsyncChannel<P>, DuplicateChannel>
    where
        P: Protocol + Default,
    {
        Self::add_with(self, P::default(), OnDirection::Responder(())).map(|dir| match dir {
            OnDirection::Responder(t) => t,
            _ => {
                panic!("internal error: should return only responder value")
            }
        })
    }

    #[must_use]
    pub fn add_with<P: Protocol>(
        &mut self,
        protocol: P,
        direction: OnDirection<()>,
    ) -> Result<OnDirection<AsyncChannel<P>>, DuplicateChannel> {
        let create_initiator =
            || AsyncChannel::new(Direction::Initiator, protocol, self.mux_notify.clone());
        let create_responder =
            || AsyncChannel::new(Direction::Responder, protocol, self.mux_notify.clone());

        let channel_id = P::PROTOCOL_NUMBER;
        let channel = match direction {
            OnDirection::Initiator(()) => OnDirection::Initiator(create_initiator()),
            OnDirection::Responder(()) => OnDirection::Responder(create_responder()),
            OnDirection::InitiatorAndResponder((), ()) => {
                OnDirection::InitiatorAndResponder(create_initiator(), create_responder())
            }
        };
        self.channels
            .add(channel_id, channel.map(|c| c.channel.clone()))?;
        Ok(channel)
    }

    #[must_use]
    pub fn finalize(self) -> ChannelsMap<OnDirection<AsyncRawChannel>> {
        self.channels.finalize()
    }
}
