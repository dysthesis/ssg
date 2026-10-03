use std::{
    fmt,
    io::{self, Read, Write},
    path::PathBuf,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::fs::{Dir, Fs, Outcome};

/// A 32-byte content digest used to address an object in a [`Store`].
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Id([u8; 32]);

impl Id {
    /// Construct a digest value without checking membership or content
    /// integrity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the digest bytes in their original order.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = encode_hex(self.as_bytes());
        f.write_str(std::str::from_utf8(&hex).expect("hex encoding is ASCII"))
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Id").field(&format_args!("{self}")).finish()
    }
}

impl FromStr for Id {
    type Err = Error;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        let input = input.as_bytes();
        if input.len() != 64 {
            return Err(Error::InvalidIdLength {
                actual: input.len(),
            });
        }

        let mut bytes = [0; 32];
        for (i, pair) in input.as_chunks::<2>().0.iter().enumerate() {
            let high = decode_hex_digit(pair[0]).ok_or(Error::InvalidIdHex { index: i * 2 })?;
            let low = decode_hex_digit(pair[1]).ok_or(Error::InvalidIdHex { index: i * 2 + 1 })?;
            bytes[i] = high << 4 | low;
        }
        Ok(Self::from_bytes(bytes))
    }
}

fn encode_hex(bytes: &[u8; 32]) -> [u8; 64] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = [0; 64];
    for (i, &byte) in bytes.iter().enumerate() {
        hex[i * 2] = DIGITS[(byte >> 4) as usize];
        hex[i * 2 + 1] = DIGITS[(byte & 0x0f) as usize];
    }
    hex
}

fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Errors from parsing IDs or accessing stored objects.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Expected 64 hexadecimal bytes for an object ID, found {actual}.")]
    InvalidIdLength { actual: usize },
    #[error("Invalid hexadecimal digit in an object ID at byte {index}.")]
    InvalidIdHex { index: usize },
    #[error("The path {path} cannot be found, or is inaccessible.")]
    PathNotFound { path: PathBuf },
    #[error("Failed to put object {hash}")]
    PutError {
        hash: Id,
        #[source]
        error: io::Error,
    },
    #[error("Failed to get object {hash}")]
    GetError {
        hash: Id,
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
    /// Read the complete object and verify its BLAKE3 digest before returning bytes.
    ///
    /// A valid [`Id`] may be absent; the digest value alone does not establish
    /// store membership or content integrity.
    pub fn get(&self, key: Id) -> Result<Vec<u8>> {
        self.inner.get(key)
    }
    /// Store the complete input bytes, returning their BLAKE3 digest as an [`Id`].
    ///
    /// If the object already exists, verify its digest and compare all resident
    /// bytes with the input before accepting the duplicate.
    pub fn put(&self, bytes: &[u8]) -> Result<Id> {
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

    fn get(&self, key: Id) -> Result<Vec<u8>> {
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

    fn read_object(mut reader: F::Reader, hash: Id) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        if blake3::hash(&bytes).as_bytes() != hash.as_bytes() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "object hash mismatch",
            ));
        }
        Ok(bytes)
    }

    fn put(&self, bytes: &[u8]) -> Result<Id> {
        let hash = Id::from_bytes(*blake3::hash(bytes).as_bytes());
        match self.put_inner(bytes, hash) {
            Ok(_) => Ok(hash),
            Err(error) => Err(Error::PutError { hash, error }),
        }
    }

    // TODO: maybe merge this back into `put`?
    #[inline]
    fn put_inner(&self, bytes: &[u8], hash: Id) -> io::Result<()> {
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
    fn object_path(&self, hash: Id) -> PathBuf {
        let hex = encode_hex(hash.as_bytes());
        let hex = std::str::from_utf8(&hex).expect("hex encoding is ASCII");
        let mut path = PathBuf::with_capacity(hex.len() + 1);
        path.push(&hex[..2]);
        path.push(&hex[2..]);
        path
    }
}

#[cfg(test)]
mod tests;
