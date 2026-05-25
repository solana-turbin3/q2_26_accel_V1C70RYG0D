//! Generic storage container with pluggable serialization formats.
//!
//! Supports Borsh, Bincode (the "Wincode" referenced in the spec), and JSON
//! through a single `Serializer` trait and a `Storage<T, S>` container that
//! holds the serialized bytes internally and uses `PhantomData<T>` to keep
//! the value type information without storing the value itself.

use std::marker::PhantomData;

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("no data has been stored yet")]
    Empty,
    #[error("borsh error: {0}")]
    Borsh(#[from] std::io::Error),
    #[error("bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// A pluggable serialization format.
pub trait Serializer {
    fn to_bytes<T: BorshSerialize + Serialize>(&self, value: &T) -> Result<Vec<u8>, StorageError>;
    fn from_bytes<T: BorshDeserialize + DeserializeOwned>(
        &self,
        bytes: &[u8],
    ) -> Result<T, StorageError>;
}

/// Borsh binary format.
pub struct Borsh;

impl Serializer for Borsh {
    fn to_bytes<T: BorshSerialize + Serialize>(&self, value: &T) -> Result<Vec<u8>, StorageError> {
        Ok(borsh::to_vec(value)?)
    }

    fn from_bytes<T: BorshDeserialize + DeserializeOwned>(
        &self,
        bytes: &[u8],
    ) -> Result<T, StorageError> {
        Ok(T::try_from_slice(bytes)?)
    }
}

/// Bincode binary format (the spec called this "Wincode").
pub struct Wincode;

impl Serializer for Wincode {
    fn to_bytes<T: BorshSerialize + Serialize>(&self, value: &T) -> Result<Vec<u8>, StorageError> {
        Ok(bincode::serialize(value)?)
    }

    fn from_bytes<T: BorshDeserialize + DeserializeOwned>(
        &self,
        bytes: &[u8],
    ) -> Result<T, StorageError> {
        Ok(bincode::deserialize(bytes)?)
    }
}

/// JSON text format.
pub struct Json;

impl Serializer for Json {
    fn to_bytes<T: BorshSerialize + Serialize>(&self, value: &T) -> Result<Vec<u8>, StorageError> {
        Ok(serde_json::to_vec(value)?)
    }

    fn from_bytes<T: BorshDeserialize + DeserializeOwned>(
        &self,
        bytes: &[u8],
    ) -> Result<T, StorageError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

/// Generic byte-backed storage parameterized by a value type `T` and a
/// concrete serializer `S`.  The value is never kept in memory in its
/// typed form — only as serialized bytes — while `PhantomData<T>` keeps
/// the compiler aware of which type the bytes represent.
pub struct Storage<T, S>
where
    T: BorshSerialize + BorshDeserialize + Serialize + DeserializeOwned,
    S: Serializer,
{
    bytes: Option<Vec<u8>>,
    serializer: S,
    _marker: PhantomData<T>,
}

impl<T, S> Storage<T, S>
where
    T: BorshSerialize + BorshDeserialize + Serialize + DeserializeOwned,
    S: Serializer,
{
    /// Create empty storage backed by the given serializer.
    pub fn new(serializer: S) -> Self {
        Self {
            bytes: None,
            serializer,
            _marker: PhantomData,
        }
    }

    /// Serialize and replace any previously stored value.
    pub fn save(&mut self, value: &T) -> Result<(), StorageError> {
        self.bytes = Some(self.serializer.to_bytes(value)?);
        Ok(())
    }

    /// Deserialize and return the stored value.
    pub fn load(&self) -> Result<T, StorageError> {
        let bytes = self.bytes.as_deref().ok_or(StorageError::Empty)?;
        self.serializer.from_bytes(bytes)
    }

    /// Whether anything has been stored.
    pub fn has_data(&self) -> bool {
        self.bytes.is_some()
    }

    /// Raw serialized bytes (useful for inspection / cross-format migration).
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }

    /// Re-encode the stored bytes from this storage into another serializer
    /// format, returning fresh storage in that new format.  Bonus #1.
    pub fn convert<S2>(&self, other: S2) -> Result<Storage<T, S2>, StorageError>
    where
        S2: Serializer,
    {
        let mut out = Storage::<T, S2>::new(other);
        if let Some(_) = &self.bytes {
            let value: T = self.load()?;
            out.save(&value)?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(
        Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, serde::Deserialize,
    )]
    struct Person {
        name: String,
        age: u32,
    }

    fn sample() -> Person {
        Person {
            name: "André".to_string(),
            age: 30,
        }
    }

    #[test]
    fn borsh_roundtrip() {
        let mut s: Storage<Person, _> = Storage::new(Borsh);
        assert!(!s.has_data());
        s.save(&sample()).unwrap();
        assert!(s.has_data());
        assert_eq!(s.load().unwrap(), sample());
    }

    #[test]
    fn wincode_roundtrip() {
        let mut s: Storage<Person, _> = Storage::new(Wincode);
        s.save(&sample()).unwrap();
        assert_eq!(s.load().unwrap(), sample());
    }

    #[test]
    fn json_roundtrip() {
        let mut s: Storage<Person, _> = Storage::new(Json);
        s.save(&sample()).unwrap();
        assert_eq!(s.load().unwrap(), sample());
        // JSON is human readable — confirm it.
        let text = std::str::from_utf8(s.bytes().unwrap()).unwrap();
        assert!(text.contains("André") && text.contains("30"));
    }

    #[test]
    fn load_before_save_errors() {
        let s: Storage<Person, _> = Storage::new(Borsh);
        assert!(matches!(s.load(), Err(StorageError::Empty)));
    }

    #[test]
    fn convert_between_serializers() {
        let mut borsh_store: Storage<Person, _> = Storage::new(Borsh);
        borsh_store.save(&sample()).unwrap();

        let json_store = borsh_store.convert(Json).unwrap();
        assert_eq!(json_store.load().unwrap(), sample());

        let wincode_store = json_store.convert(Wincode).unwrap();
        assert_eq!(wincode_store.load().unwrap(), sample());
    }
}
