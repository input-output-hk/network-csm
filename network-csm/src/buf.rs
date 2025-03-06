use core::fmt;

use alloc::{vec, vec::Vec};

use crate::{CborBufValidate, cbor_buf_validate};

/// Fixed sized byte buffer
pub struct Buf {
    buf: Vec<u8>,
    pos: usize,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum BufCborReadingError {
    /// CBOR received is not a valid encoding
    InvalidCBOR,
    /// Message cannot be decoded from the CBOR
    InvalidValue(String),
}

impl fmt::Display for BufCborReadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for BufCborReadingError {}

impl Buf {
    pub fn new(size: usize) -> Self {
        Self {
            buf: vec![0_u8; size],
            pos: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.pos
    }

    pub fn empty_mut(&mut self) -> &mut [u8] {
        &mut self.buf[self.pos..]
    }

    pub fn empty_remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Return the data that can be consumed
    pub fn available(&self) -> &[u8] {
        &self.buf[0..self.pos]
    }

    /// Append data to the buffer, returning the appended size which
    /// might be less than the requested data if the space available
    /// is smaller than the request.
    pub fn append(&mut self, data: &[u8]) -> usize {
        let empty_space = self.empty_mut();
        if empty_space.len() < data.len() {
            empty_space.copy_from_slice(&data[0..empty_space.len()]);
            empty_space.len()
        } else {
            empty_space[0..data.len()].copy_from_slice(data);
            self.pos += data.len();
            data.len()
        }
    }

    /// Append data (all or nothing)
    ///
    /// if nothing is appended, then Err is returned, otherwise Ok
    pub fn append_atomic(&mut self, data: &[u8]) -> Result<(), ()> {
        let empty_space = self.empty_mut();
        if empty_space.len() < data.len() {
            return Err(());
        }
        empty_space[0..data.len()].copy_from_slice(data);
        self.pos += data.len();
        Ok(())
    }

    /// Variant of `append_atomic` that take 2 slices
    ///
    /// if nothing is appended, then Err is returned, otherwise Ok
    pub fn append_atomic2(&mut self, data1: &[u8], data2: &[u8]) -> Result<(), ()> {
        let empty_space = self.empty_mut();
        if empty_space.len() < (data1.len() + data2.len()) {
            return Err(());
        }
        empty_space[0..data1.len()].copy_from_slice(data1);
        empty_space[data1.len()..data1.len() + data2.len()].copy_from_slice(data2);
        self.pos += data1.len() + data2.len();
        Ok(())
    }

    /// Consume N bytes from the buffer, When we consume data, we move bytes data from the unconsumed data back to offset 0
    pub fn consume(&mut self, bytes: usize) {
        assert!(self.pos >= bytes);
        if self.pos > bytes {
            self.buf.copy_within(bytes..self.pos, 0);
        }
        self.pos -= bytes;
    }

    /// Consume a fully formed CBOR message if present
    pub fn consume_cbor<T: cbored::Decode>(&mut self) -> Option<Result<T, BufCborReadingError>> {
        match cbor_buf_validate(self.available()) {
            CborBufValidate::CborError => Some(Err(BufCborReadingError::InvalidCBOR)),
            CborBufValidate::NeedMore => None,
            CborBufValidate::Slice(_, sz) => {
                let data = &self.available()[0..sz];
                let mut cbor_data = cbored::Reader::new(data);
                match cbor_data.decode::<T>() {
                    Err(e) => Some(Err(BufCborReadingError::InvalidValue(format!("{}", e)))),
                    Ok(t) => {
                        self.consume(sz);
                        Some(Ok(t))
                    }
                }
            }
        }
    }
}

#[test]
fn buf_works() {
    let mut b = Buf::new(10);
    assert_eq!(b.len(), 0);
    let data1 = &[1, 2, 3, 4, 5];
    let data2 = &[6];
    let full_data = data1
        .iter()
        .cloned()
        .chain(data2.iter().cloned())
        .collect::<Vec<_>>();
    let sz_append1 = b.append(data1);
    assert_eq!(sz_append1, data1.len());
    assert_eq!(b.len(), data1.len());
    let sz_append2 = b.append(data2);
    assert_eq!(sz_append2, data2.len());
    assert_eq!(b.len(), data1.len() + data2.len());
    assert_eq!(b.available(), &full_data);
    b.consume(3);
    assert_eq!(b.len(), 3);
    assert_eq!(b.available(), &full_data[3..]);
    b.consume(3);
    assert_eq!(b.len(), 0);
}
