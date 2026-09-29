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
    pub path: PathBuf,
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

    #[cfg(test)]
    pub fn objects(&self) -> color_eyre::Result<Vec<blake3::Hash>> {
        use color_eyre::eyre::bail;
        use std::fs;
        fs::read_dir(self.path.clone())?
            .map(|entry| {
                let entry = entry?;

                if !entry.file_type()?.is_file() {
                    bail!("unexpected non-file entry: {:?}", entry.path());
                }

                let name = entry.file_name();
                blake3::Hash::from_hex(name.as_encoded_bytes()).map_err(Into::into)
            })
            .collect()
    }

    #[cfg(test)]
    fn object_path(&self, hash: blake3::Hash) -> PathBuf {
        self.path.join(hash.to_hex().as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest_derive::Arbitrary;
    use tempdir::TempDir;

    /// For modelling arbitrary actions in between our operations of interest
    #[derive(Debug, Arbitrary)]
    enum Op {
        Put(Vec<u8>),
        GetNonexistent(Vec<u8>),
        GetExisting(usize),
        Reopen,
    }

    impl Op {
        pub fn run(&self, store: &Store) -> Result<()> {
            match self {
                Op::Put(bytes) => {
                    store.put(bytes)?;
                }
                Op::GetNonexistent(bytes) => {
                    let hash = blake3::hash(bytes);
                    store.get(hash)?;
                }
                Op::GetExisting(idx) => {
                    let objects = store.objects().unwrap();
                    let clamped = idx % objects.len();
                    let key = objects.get(clamped).expect("idx is a valid index");
                    let _ = store.get(*key);
                }
                Op::Reopen => {
                    let _ = Store::init(store.path.clone());
                }
            }
            Ok(())
        }
    }
    /// Structural boundaries of BLAKE3 to catch off-by-one errors
    fn boundary_sized_bytes() -> impl Strategy<Value = Vec<u8>> {
        prop_oneof![
            8 => prop::sample::select(vec![
                0usize,
                1,
                63,
                64,
                65,
                1023,
                1024,
                1025,
                2048,
                2049,
                4096,
                4097
            ]),
            2 => 0usize..=16 * 1024,
        ]
        .prop_flat_map(|len| prop::collection::vec(any::<u8>(), len))
    }

    fn corrupted_object() -> impl Strategy<Value = (Vec<u8>, std::collections::BTreeSet<usize>)> {
        prop::collection::vec(any::<u8>(), 1..=4096).prop_flat_map(|bytes| {
            let len = bytes.len();

            (Just(bytes), prop::collection::btree_set(0..len, 1..=len))
        })
    }
    fn arb_hash() -> impl Strategy<Value = blake3::Hash> {
        any::<[u8; 32]>().prop_map(blake3::Hash::from_bytes)
    }

    proptest! {
        /// Self-explanatory, putting some bytes should return its hash
        #[test]
        fn put_returns_hash(bytes in any::<Vec<u8>>()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let key = store.put(&bytes)?;
            let expected = blake3::hash(&bytes);
            prop_assert_eq!(key, expected);
        }

        /// Putting some bytes, then getting it, should return the original
        #[test]
        fn put_get_roundtrips(bytes in any::<Vec<u8>>()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let key = store.put(&bytes)?;
            let obtained = store.get(key)?;
            prop_assert_eq!(obtained, bytes);
        }

        /// Getting a key yields data that hashes to the same key
        #[test]
        fn get_verifies_hash(bytes in any::<Vec<u8>>()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let key = store.put(&bytes)?;
            let obtained = store.get(key)?;
            let hashed = blake3::hash(&obtained);
            prop_assert_eq!(hashed, key);
        }

        /// Getting a nonexistent key yields an Error
        #[test]
        fn get_nonexistent_yields_err(bytes in any::<Vec<u8>>()) {
            // We're not going to actually put the bytes in the store,
            // we'll just hash it
            let nonexistent = blake3::hash(&bytes);
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;
            let obtained = store.get(nonexistent);
            prop_assert!(obtained.is_err());
        }

        /// We obtain the same object that we put in, even after arbitrary
        /// operations
        #[test]
        fn object_id_is_immutable(bytes in any::<Vec<u8>>(), ops in any::<Vec<Op>>()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;
            let key = store.put(&bytes)?;

            for op in ops {
                let _ = op.run(&store);
            }
            drop(store);

            let store = Store::init(tmp.path().to_path_buf())?;
            let obtained = store.get(key)?;
            prop_assert_eq!(obtained, bytes);
        }
        /// Roundtrip test again, but for boundary sizes in particular for
        /// off-by-one errors
        #[test]
        fn objects_roundtrip_in_boundary(bytes in boundary_sized_bytes()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;
            let hash = store.put(&bytes).unwrap();
            let actual = store.get(hash).unwrap();
            prop_assert_eq!(actual, bytes);
        }

        /// Putting something multiple times yields the same key
        #[test]
        fn put_idempotence(bytes in any::<Vec<u8>>(), reps in 2usize..1000) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let attempts = (0..reps).map(|_| store.put(&bytes).unwrap()).collect::<Vec<_>>();
            // All attempts must yield the same
            prop_assert!(attempts.windows(2).all(|w| w[0] == w[1]));
        }

        /// Getting something multiple times yields the same key
        #[test]
        fn get_idemptotence(bytes in any::<Vec<u8>>(), reps in 2usize..1000) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;
            let key = store.put(&bytes)?;

            let attempts = (0..reps).map(|_| store.get(key).unwrap()).collect::<Vec<_>>();
            // All attempts must yield the same
            prop_assert!(attempts.windows(2).all(|w| w[0] == w[1]));
        }


        /// Corrupting the file backing makes `get` yield an error
        #[test]
        fn replacement_with_different_content_is_rejected(
            original in any::<Vec<u8>>(),
            replacement in any::<Vec<u8>>(),
        ) {
            prop_assume!(blake3::hash(&original) != blake3::hash(&replacement));

            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let hash = store.put(&original).unwrap();

            std::fs::write(
                store.object_path(hash),
                &replacement,
            ).unwrap();

            drop(store);
            let store = Store::init(tmp.path().to_path_buf())?;

            prop_assert!(store.get(hash).is_err());
        }

        /// Reject truncated object files
        #[test]
        fn any_truncation_is_rejected(
            bytes in prop::collection::vec(any::<u8>(), 1..4096),
            numerator in any::<usize>(),
        ) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let hash = store.put(&bytes).unwrap();
            let path = store.object_path(hash);

            let new_len = numerator % bytes.len();

            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap();

            file.set_len(new_len as u64).unwrap();

            drop(store);
            let store = Store::init(tmp.path().to_path_buf())?;

            prop_assert!(store.get(hash).is_err());
        }
        /// Arbitrarily mutate bytes in the file and ensure that `get` rejects
        /// it
        #[test]
        fn arbitrary_byte_mutations_are_rejected(
            (bytes, indices) in corrupted_object(),
        ) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let hash = store.put(&bytes).unwrap();
            let path = store.object_path(hash);

            let mut corrupted = bytes.clone();

            for index in indices {
                corrupted[index] = corrupted[index].wrapping_add(1);
            }

            std::fs::write(path, &corrupted).unwrap();

            prop_assert!(store.get(hash).is_err());
        }

        /// Swap two objects' files and ensure `get` rejects them as well
        #[test]
        fn wrong_object_at_hash_path_is_rejected(
            x in any::<Vec<u8>>(),
            y in any::<Vec<u8>>(),
        ) {
            let x_hash = blake3::hash(&x);
            let y_hash = blake3::hash(&y);

            prop_assume!(x_hash != y_hash);

            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let actual_hash = store.put(&x).unwrap();
            prop_assert_eq!(actual_hash, x_hash);

            std::fs::write(
                store.object_path(x_hash),
                &y,
            )
            .unwrap();

            let result = store.get(x_hash);

            prop_assert!(result.is_err());
        }
    }
}
