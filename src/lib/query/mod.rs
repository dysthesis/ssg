use std::{borrow::Cow, collections::HashMap, hash::Hash};

use crate::store::{self, Store};

pub mod read;

/// A persistent identifier for a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key([u8; 32]);
impl From<Key> for [u8; 32] {
    fn from(value: Key) -> Self {
        value.0
    }
}

/// An execution-local identifier of a [`Query`], that is also an index to the
/// corresponding [`Query`] in [`Registry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(usize);
impl From<Id> for usize {
    fn from(value: Id) -> Self {
        value.0
    }
}

const HASH_DOMAIN: &[u8] = b"ssg/query\0";

// is 'static unavoidable?
pub trait Query: 'static + Hash {
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
    ///
    /// Note that this method is not responsible for caching. It will just
    /// eagerly run the build again, and therefore should not be called by
    /// anything other than [`crate::ctx::Ctx`]
    fn query(&self, store: &Store) -> store::Id;
}

/// A dyn-compatible interface for [`Query`], containing basically the only
/// functions it needs.
pub trait Erased {
    fn key(&self) -> Key;
    fn query(&self, store: &Store) -> store::Id;
}

impl<Q: Query> Erased for Q {
    #[inline]
    fn key(&self) -> Key {
        Query::key(self)
    }

    #[inline]
    fn query(&self, store: &Store) -> store::Id {
        Query::query(self, store)
    }
}

/// For an _array_ of bytes, hash the _length_ of said array before the contents
fn hash_bytes(h: &mut blake3::Hasher, bytes: &[u8]) {
    h.update(&(bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}

/// A list of all of the queries inspected durin this run.
#[derive(Default)]
pub struct Registry {
    inner: Vec<Box<dyn Erased>>, // TODO: figure out a more efficient repr
    by_key: HashMap<Key, Id>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            inner: Vec::new(),
            by_key: HashMap::new(),
        }
    }

    /// Register `query`, or return the existing ID if already registered.
    pub fn put<Q: Query>(&mut self, query: Q) -> Id {
        let key = query.key();

        if let Some(&id) = self.by_key.get(&key) {
            return id;
        }

        let id = Id(self.inner.len());

        self.inner.push(Box::new(query));
        self.by_key.insert(key, id);

        id
    }

    /// Get the query associated with `id`
    pub fn get(&self, id: Id) -> Option<&dyn Erased> {
        self.inner.get(id.0).map(|v| &**v)
    }
}
