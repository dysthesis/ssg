use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The path {path} cannot be found, or is inaccessible.")]
    PathNotFound { path: PathBuf },
}

type Result<T> = std::result::Result<T, Error>;

/// A content-addressable storage used to store intermediate results of the
/// build
pub struct Store {
    path: PathBuf,
}

impl Store {
    /// Initialise a given path as the object storage path
    pub fn init(path: PathBuf) -> Result<Self> {
        todo!()
    }
    /// Get the bytes stored that is associated with the given key
    pub fn get(&self, key: blake3::Hash) -> Result<Vec<u8>> {
        todo!()
    }
    /// Put some arbitrary bytes into the store, returning its key.
    pub fn put(&self, bytes: &[u8]) -> Result<blake3::Hash> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempdir::TempDir;

    proptest! {
        /// Self-explanatory, putting some bytes should return its hash
        #[test]
        fn put_returns_hash(bytes in any::<Vec<u8>>()) {
            let tmp = TempDir::new("test-put-returns-hash")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let key = store.put(&bytes)?;
            let expected = blake3::hash(&bytes);
            prop_assert_eq!(key, expected);
        }
    }
}
