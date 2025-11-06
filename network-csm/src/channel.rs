use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::buf::Buf;
use crate::cbor_helper::{CborBufValidate, cbor_buf_validate};

#[derive(Clone)]
pub struct Channel {
    inner: Arc<ChannelImpl>,
}

pub struct ChannelImpl {
    recv_data: Mutex<Buf>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ReadMessageError(Option<String>);

impl fmt::Display for ReadMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for ReadMessageError {}

impl Channel {
    pub fn new(size: usize) -> Self {
        let inner = Arc::new(ChannelImpl {
            recv_data: Mutex::new(Buf::new(size)),
        });
        Self { inner }
    }

    pub fn try_buf_received(&self) -> Option<MutexGuard<'_, Buf>> {
        let lock = self.inner.recv_data.lock().unwrap();
        if lock.empty_is_empty() {
            None
        } else {
            Some(lock)
        }
    }

    pub fn buf_received(&self) -> MutexGuard<'_, Buf> {
        self.inner.recv_data.lock().unwrap()
    }

    pub fn push_bytes(&self, data: &[u8]) -> Option<usize> {
        let mut buf = self.try_buf_received()?;
        Some(buf.append(data))
    }

    pub fn pop_message<T: cbored::Decode>(&mut self) -> Option<Result<T, ReadMessageError>> {
        let mut buf = self.inner.recv_data.lock().unwrap();
        match cbor_buf_validate(buf.available()) {
            CborBufValidate::CborError => Some(Err(ReadMessageError(None))),
            CborBufValidate::NeedMore => {
                if buf.empty_is_empty() {
                    Some(Err(ReadMessageError(Some(format!(
                        "Buffer is full ({max} bytes). And there is no finished message yet.",
                        max = buf.maximum_capacity()
                    )))))
                } else {
                    None
                }
            }
            CborBufValidate::Slice(_, sz) => {
                let data = &buf.available()[0..sz];
                let mut cbor_data = cbored::Reader::new(data);
                match cbor_data.decode::<T>() {
                    Err(e) => Some(Err(ReadMessageError(Some(format!("{}", e))))),
                    Ok(t) => {
                        buf.consume(sz);
                        Some(Ok(t))
                    }
                }
            }
        }
    }
}
