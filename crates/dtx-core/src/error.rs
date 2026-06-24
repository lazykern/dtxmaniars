use std::result::Result as StdResult;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DtxError {
    #[error("I/O error reading DTX file: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("invalid line {line}: {message}")]
    InvalidLine { line: usize, message: String },

    #[error("invalid channel value {value} in line {line}")]
    InvalidChannel { line: usize, value: u8 },

    #[error("invalid measure {value} in line {line}")]
    InvalidMeasure { line: usize, value: u16 },

    #[error("chip data length mismatch in line {line}: expected {expected} chars, got {actual}")]
    ChipDataLength {
        line: usize,
        expected: usize,
        actual: usize,
    },
}

pub type Result<T> = StdResult<T, DtxError>;
