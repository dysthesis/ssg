use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

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
        let hash = blake3::hash(bytes);
        let writer = |file: &mut fs::File, bytes: &[u8]| file.write_all(bytes);
        match self.put_inner(bytes, hash, writer) {
            Ok(_) => Ok(hash),
            Err(error) => Err(Error::PutError { hash, error }),
        }
    }

    fn put_inner<W>(&self, bytes: &[u8], hash: blake3::Hash, writer: W) -> io::Result<()>
    where
        W: FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
    {
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

    #[inline]
    fn object_path(&self, hash: blake3::Hash) -> PathBuf {
        let hex = hash.to_hex();

        self.path.join(&hex[..2]).join(&hex[2..])
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Barrier,
        },
        thread,
    };

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
    fn hash_with_leading_zeroes() -> impl Strategy<Value = blake3::Hash> {
        (1usize..32, any::<[u8; 32]>()).prop_map(|(zeroes, mut bytes)| {
            bytes[..zeroes].fill(0);
            blake3::Hash::from_bytes(bytes)
        })
    }

    fn object_and_proper_prefix() -> impl Strategy<Value = (Vec<u8>, usize)> {
        prop::collection::vec(any::<u8>(), 1..=8192).prop_flat_map(|bytes| {
            let len = bytes.len();

            (Just(bytes), 0..len)
        })
    }
    fn injected_error() -> std::io::Error {
        std::io::Error::other("injected write failure")
    }
    fn distinct_objects() -> impl Strategy<Value = Vec<Vec<u8>>> {
        prop::collection::btree_set(prop::collection::vec(any::<u8>(), 0..=4096), 2..=16)
            .prop_map(|set| set.into_iter().collect())
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

        /// If two hashes differ, their object paths differ
        #[test]
        fn object_path_is_injective(
            left in arb_hash(),
            right in arb_hash(),
        ) {
            prop_assume!(left != right);

            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let left_path = store.object_path(left);
            let right_path = store.object_path(right);

            prop_assert_ne!(left_path, right_path);
        }

        /// Test that leading zeroes are preserved in the object path
        #[test]
        fn object_path_preserves_leading_zeroes(
            hash in hash_with_leading_zeroes(),
        ) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;

            let path = store.object_path(hash);

            let filename = path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();

            prop_assert_eq!(filename.len(), 64);

            let decoded = blake3::Hash::from_hex(filename).unwrap();

            prop_assert_eq!(decoded, hash);
        }

        #[test]
        fn sharded_path_preserves_full_hash(
            hash in arb_hash(),
        ) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;
            let path = store.object_path(hash);

            let relative =
                path.strip_prefix(store.path).unwrap();

            let mut components =
                relative.components();

            let prefix = components
                .next().unwrap()
                .as_os_str()
                .to_str().unwrap();

            let suffix = components
                .next().unwrap()
                .as_os_str()
                .to_str().unwrap();

            prop_assert!(
                components.next().is_none()
            );

            let reconstructed =
                format!("{prefix}{suffix}");

            prop_assert_eq!(
                blake3::Hash::from_hex(&reconstructed) .unwrap(),
                hash,
            );
        }

        /// Use a fault-injection writer that terminates after a pre-determined
        /// prefix, and test that `put` never published the truncated object.
        #[test]
        fn incomplete_object_is_never_published(
            (bytes, prefix_len) in object_and_proper_prefix(),
        ) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(tmp.path().to_path_buf())?;
            let hash = blake3::hash(&bytes);

            let writer = |file: &mut fs::File, bytes: &[u8]| {
                file.write_all(&bytes[..prefix_len])?;
                Err(injected_error())
            };
            let result = store.put_inner(
                &bytes,
                hash,
                writer
            );

            prop_assert!(result.is_err());

            // The logical object must not exist.
            prop_assert!(store.get(hash).is_err());
        }
        /// Multiple concurrent puts of the same bytes yields correct, identical
        /// objects and keys
        #[test]
        fn concurrent_identical_puts_converge(
            bytes in prop::collection::vec(any::<u8>(), 0..=8192),
            writers in 2usize..=16,
        ) {
            let temp = TempDir::new("store").unwrap();

            let store = Arc::new(
                Store::init(temp.path().to_path_buf()).unwrap()
            );

            let barrier = Arc::new(Barrier::new(writers + 1));

            let handles: Vec<_> = (0..writers)
                .map(|_| {
                    let store = Arc::clone(&store);
                    let barrier = Arc::clone(&barrier);
                    let bytes = bytes.clone();

                    thread::spawn(move || {
                        barrier.wait();
                        store.put(&bytes)
                    })
                })
                .collect();

            // Release all writers together.
            barrier.wait();

            let expected = blake3::hash(&bytes);

            for handle in handles {
                let actual = handle
                    .join()
                    .expect("writer panicked")
                    .expect("put failed");

                prop_assert_eq!(actual, expected);
            }

            prop_assert_eq!(
                store.get(expected).unwrap(),
                bytes,
            );
        }
        /// Puts remain correct even when concurrent, different puts occur
        #[test]
        fn concurrent_distinct_puts_do_not_interfere(
            objects in distinct_objects(),
        ) {
            let temp = TempDir::new("store").unwrap();

            let store = Arc::new(
                Store::init(temp.path().to_path_buf()).unwrap()
            );

            let expected: Vec<_> = objects
                .iter()
                .map(|bytes| blake3::hash(bytes))
                .collect();

            // Exclude an actual cryptographic collision from the property.
            let unique_hashes: std::collections::HashSet<_> =
                expected.iter().copied().collect();

            prop_assume!(unique_hashes.len() == objects.len());

            let barrier =
                Arc::new(Barrier::new(objects.len() + 1));

            let handles: Vec<_> = objects
                .iter()
                .cloned()
                .map(|bytes| {
                    let store = Arc::clone(&store);
                    let barrier = Arc::clone(&barrier);

                    thread::spawn(move || {
                        barrier.wait();

                        let hash = store.put(&bytes)?;

                        Ok::<_, Error>((hash, bytes))
                    })
                })
                .collect();

            barrier.wait();

            for handle in handles {
                let (hash, bytes) = handle
                    .join()
                    .expect("writer panicked")
                    .expect("put failed");

                prop_assert_eq!(
                    hash,
                    blake3::hash(&bytes)
                );
            }

            // Verify the whole logical store after all racing writes.
            for (bytes, hash) in objects.iter().zip(expected) {
                prop_assert_eq!(
                    store.get(hash).unwrap(),
                    bytes.clone(),
                );
            }
        }

        /// During a racing publication, every read is either NotFound or the complete correct object
        #[test]
        fn concurrent_get_never_observes_partial_object(
            bytes in prop::collection::vec(any::<u8>(), 1..=64 * 1024),
            readers in 1usize..=8,
        ) {
            let temp = TempDir::new("store").unwrap();

            let store = Arc::new(
                Store::init(temp.path().to_path_buf()).unwrap()
            );

            let hash = blake3::hash(&bytes);

            let start =
                Arc::new(Barrier::new(readers + 2));

            let finished =
                Arc::new(AtomicBool::new(false));

            let reader_handles: Vec<_> = (0..readers)
                .map(|_| {
                    let store = Arc::clone(&store);
                    let start = Arc::clone(&start);
                    let finished = Arc::clone(&finished);
                    let expected = bytes.clone();

                    thread::spawn(move || {
                        start.wait();

                        while !finished.load(Ordering::Acquire) {
                            match store.get(hash) {
                                Ok(actual) => {
                                    assert_eq!(actual, expected);
                                    assert_eq!(
                                        blake3::hash(&actual),
                                        hash
                                    );
                                }

                                Err(Error::PathNotFound { .. }) => {
                                    // Valid before publication.
                                }

                                Err(error) => {
                                    panic!(
                                        "reader observed invalid intermediate state: \
                                         {error:?}"
                                    );
                                }
                            }

                            thread::yield_now();
                        }
                    })
                })
                .collect();

            let writer_store = Arc::clone(&store);
            let writer_start = Arc::clone(&start);
            let writer_finished = Arc::clone(&finished);
            let writer_bytes = bytes.clone();

            let writer = thread::spawn(move || {
                writer_start.wait();

                let result = writer_store.put(&writer_bytes);

                writer_finished.store(true, Ordering::Release);

                result
            });

            // Release all readers and writer simultaneously.
            start.wait();

            let actual_hash = writer
                .join()
                .expect("writer panicked")
                .expect("put failed");

            prop_assert_eq!(actual_hash, hash);

            for reader in reader_handles {
                reader.join().expect("reader panicked");
            }

            // Once put has returned, absence is no longer permitted.
            let actual = store.get(hash).unwrap();

            prop_assert_eq!(&actual, &bytes);
            prop_assert_eq!(blake3::hash(&actual), hash);
        }

        /// You cannot have two writers writing to the same file. But crashing
        /// one writer should not interfere with the other.
        #[test]
        fn crashed_duplicate_writer_does_not_poison_survivor(
            bytes in prop::collection::vec(any::<u8>(), 0..=8192),
        ) {
            let temp = TempDir::new("store").unwrap();

            let root = temp.path().to_path_buf();
            let input = root.join("input");

            std::fs::write(&input, &bytes).unwrap();

            let mut child = Command::new(
                std::env::current_exe().unwrap()
            )
            .arg("duplicate_put_crash_child")
            .arg("--ignored")
            .arg("--exact")
            .env("STORE_ROOT", &root)
            .env("STORE_INPUT", &input)
            .env("STORE_PAUSEPOINT", "before-rename")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

            let stdout = child.stdout.take().unwrap();
            let mut stdout = BufReader::new(stdout);

            let mut line = String::new();
            stdout.read_line(&mut line).unwrap();

            prop_assert_eq!(line.trim(), "READY");

            // The first writer now has an abandoned candidate immediately
            // before publication.
            let store = Store::init(root.clone()).unwrap();

            let expected = blake3::hash(&bytes);

            // Kill the first writer without cleanup.
            child.kill().unwrap();
            child.wait().unwrap();

            // A second writer must still be able to establish the object.
            let actual = store.put(&bytes).unwrap();

            prop_assert_eq!(actual, expected);

            drop(store);

            // Verify from a fresh handle, not process-local state.
            let store = Store::init(root).unwrap();

            prop_assert_eq!(
                store.get(expected).unwrap(),
                bytes,
            );
        }
    }
}
