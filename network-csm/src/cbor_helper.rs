use cbored::validate::ValidateError;

pub enum CborBufValidate<'a> {
    CborError,
    NeedMore,
    Slice(&'a cbored::CborSlice, usize),
}

pub fn cbor_buf_validate<'a>(data: &'a [u8]) -> CborBufValidate<'a> {
    if data.is_empty() {
        return CborBufValidate::NeedMore;
    }
    let mut validator = cbored::validate::Validator::new(data);
    match validator.next() {
        Err(ValidateError::DataMissing(_)) => CborBufValidate::NeedMore,
        Err(ValidateError::LeadError(_)) | Err(ValidateError::StateError(_)) => {
            CborBufValidate::CborError
        }
        Ok((slice, bytes)) => return CborBufValidate::Slice(slice, bytes),
    }
}
