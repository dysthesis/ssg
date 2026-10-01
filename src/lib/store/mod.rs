use std::{
    io::{self, Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::fs::{Dir, Fs, Outcome};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The path {path} cannot be found, or is inaccessible.")]
    PathNotFound { path: PathBuf },
    #[error("Failed to put object {hash}")]
    PutError {
        hash: blake3::Hash,
        #[source]
        error: io::Error,
    },
    #[error("Failed to get object {hash}")]
    GetError {
        hash: blake3::Hash,
        #[source]
        error: io::Error,
    },
}

type Result<T> = std::result::Result<T, Error>;

/// A content-addressable storage used to store intermediate results of the
/// build
pub struct Store {
    /// Store is basically just a wrapper around [`Backend`]; this is necessary
    /// because we only want to make the `Fs` backing replaceable for testing
    /// purposes; we should use [`Dir`] for actual prod.
    inner: Backend<Dir>,
}

impl Store {
    /// Initialise an opened directory as the object storage root
    pub fn init(dir: Dir) -> Result<Self> {
        Backend::init_with_fs(dir).map(|inner| Self { inner })
    }
    /// Get the bytes stored that is associated with the given key
    pub fn get(&self, key: blake3::Hash) -> Result<Vec<u8>> {
        self.inner.get(key)
    }
    /// Put some arbitrary bytes into the store, returning its key.
    pub fn put(&self, bytes: &[u8]) -> Result<blake3::Hash> {
        self.inner.put(bytes)
    }
}

struct Backend<F> {
    dir: F,
}

impl<F: Fs> Backend<F> {
    fn init_with_fs(dir: F) -> Result<Self> {
        Ok(Self { dir })
    }

    fn get(&self, key: blake3::Hash) -> Result<Vec<u8>> {
        let reader = self
            .dir
            .open_read(&self.object_path(key))
            .map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    Error::PathNotFound {
                        path: self.object_path(key),
                    }
                } else {
                    Error::GetError { hash: key, error }
                }
            })?;
        Self::read_object(reader, key).map_err(|error| Error::GetError { hash: key, error })
    }

    fn read_object(mut reader: F::Reader, hash: blake3::Hash) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        if blake3::hash(&bytes) != hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "object hash mismatch",
            ));
        }
        Ok(bytes)
    }

    fn put(&self, bytes: &[u8]) -> Result<blake3::Hash> {
        let hash = blake3::hash(bytes);
        match self.put_inner(bytes, hash) {
            Ok(_) => Ok(hash),
            Err(error) => Err(Error::PutError { hash, error }),
        }
    }

    // TODO: maybe merge this back into `put`?
    #[inline]
    fn put_inner(&self, bytes: &[u8], hash: blake3::Hash) -> io::Result<()> {
        // Find the next free tempfile number
        static NEXT_CANDIDATE: AtomicU64 = AtomicU64::new(0);

        let final_path = self.object_path(hash);
        let shard = final_path.parent().unwrap();
        self.dir.create_dir_all(shard)?;
        let (candidate, mut writer) = loop {
            let id = NEXT_CANDIDATE.fetch_add(1, Ordering::Relaxed);
            let candidate = shard.join(format!(".candidate-{}-{id}", std::process::id()));
            match self.dir.create_new(&candidate) {
                Ok(writer) => break (candidate, writer),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };

        // Our actual atomic write logic, after finding the tempfile number
        let publication = (|| {
            writer.write_all(bytes)?;
            writer.flush()?;
            self.dir.sync(&mut writer)?;
            drop(writer);
            self.dir.commit(&candidate, &final_path)
        })();

        if matches!(publication, Ok(Outcome::Created)) {
            return Ok(());
        }

        // best-effort cleanup may leave an orphan; never scavenge
        let _ = self.dir.remove_file(&candidate);
        publication?;
        if Self::read_object(self.dir.open_read(&final_path)?, hash)? != bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "existing object differs from input",
            ));
        }
        Ok(())
    }

    #[inline]
    fn object_path(&self, hash: blake3::Hash) -> PathBuf {
        let hex = hash.to_hex();

        PathBuf::from(&hex[..2]).join(&hex[2..])
    }
}

#[cfg(test)]
mod tests;
