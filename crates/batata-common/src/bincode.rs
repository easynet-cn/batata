//! Postcard wrapper providing a bincode 1.x-style API.
//!
//! This module wraps postcard's serde backend to provide `serialize` and `deserialize`
//! functions that are drop-in replacements for bincode 1.x's API.

use serde::{Serialize, de::DeserializeOwned};

#[derive(Debug)]
/// The `EncodeError` struct.
pub struct EncodeError(postcard::Error);

#[derive(Debug)]
/// The `DecodeError` struct.
pub struct DecodeError(postcard::Error);

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for EncodeError {}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for DecodeError {}

impl From<postcard::Error> for EncodeError {
    fn from(e: postcard::Error) -> Self {
        EncodeError(e)
    }
}

impl From<postcard::Error> for DecodeError {
    fn from(e: postcard::Error) -> Self {
        DecodeError(e)
    }
}

/// Serialize a value into a bincode-encoded byte vector.
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, EncodeError> {
    postcard::to_stdvec(value).map_err(EncodeError::from)
}

/// Deserialize a bincode-encoded byte vector into a value.
pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    postcard::from_bytes(bytes).map_err(DecodeError::from)
}
