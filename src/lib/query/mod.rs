use std::borrow::Cow;

use crate::store::{self, Store};

/// A persistent identifier for a query.
pub struct Key([u8; 32]);
impl From<Key> for [u8; 32] {
    fn from(value: Key) -> Self {
        value.0
    }
}

const HASH_DOMAIN: &[u8] = b"ssg/query\0";

pub trait Query {
    // We need some way to have persistent identity for these queries in order
    // to keep track of traces

    const NAME: &'static str;
    const VERSION: u64;

    /// Return a byte-representation of the query parameters
    fn params(&self) -> Vec<Cow<'_, [u8]>>; // Cow is used for mixing borrowed
                                            // and owned values

    fn key(&self) -> Key {
        let mut h = blake3::Hasher::new();

        h.update(HASH_DOMAIN);
        hash_bytes(&mut h, Self::NAME.as_bytes());
        h.update(&Self::VERSION.to_le_bytes());

        for param in self.params() {
            hash_bytes(&mut h, &param);
        }
        Key(*h.finalize().as_bytes())
    }

    /// The actual method that produces some value, represented as an object in
    /// the store.
    fn query(&self, store: &Store) -> store::Id;
}

/// For an _array_ of bytes, hash the _length_ of said array before the contents
fn hash_bytes(h: &mut blake3::Hasher, bytes: &[u8]) {
    h.update(&(bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}
