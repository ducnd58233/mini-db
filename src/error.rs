use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Key not found")]
    KeyNotFound,

    #[error("Node is full")]
    NodeFull,

    #[error("Invalid page type: {0}")]
    InvalidPageType(u8),

    #[error("Key size exceeds maximum: {0} > {1}")]
    KeySizeExceeded(usize, usize),

    #[error("Value size exceeds maximum: {0} > {1}")]
    ValueSizeExceed(usize, usize),

    #[error("Invalid data format: {0}")]
    InvalidFormat(String),
}

pub type Result<T> = std::result::Result<T, DbError>;
