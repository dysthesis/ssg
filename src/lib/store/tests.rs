    use std::{
        fs::File,
        io::Write,
        path::Path,
        process::{Child, Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc, Barrier, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::fs::Outcome;
    use proptest::prelude::*;
    use proptest_derive::Arbitrary;
    use tempdir::TempDir;

    type Store = Backend<TestFs>;

    impl Store {
        fn init(dir: Dir) -> Result<Self> {
            Self::init_with_fs(TestFs::new(dir))
        }
    }

    struct TestFs {
        dir: Dir,
        write_limit: Option<usize>,
        before_rename: Option<PathBuf>,
    }

    impl TestFs {
        fn new(dir: Dir) -> Self {
            Self {
                dir,
                write_limit: None,
                before_rename: None,
            }
        }

        fn pause_before_publication(&self, from: &Path, to: &Path) -> io::Result<()> {
            if let Some(ready) = &self.before_rename {
                // Let Dir enforce capability/path checks before signalling.
                self.dir.metadata(from)?;
                match self.dir.metadata(to) {
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
                let pending = ready.with_extension("pending");
                std::fs::write(&pending, from.as_os_str().as_encoded_bytes())?;
                std::fs::rename(pending, ready)?;
                loop {
                    thread::park();
                }
            }
            Ok(())
        }
    }

    struct TestWriter {
        file: std::fs::File,
        remaining: Option<usize>,
    }

    impl Write for TestWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.is_empty() {
                return Ok(0);
            }
            let len = match self.remaining {
                Some(0) => return Err(injected_error()),
                Some(remaining) => bytes.len().min(remaining),
                None => bytes.len(),
            };
            let written = self.file.write(&bytes[..len])?;
            if let Some(remaining) = &mut self.remaining {
                *remaining -= written;
            }
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.file.flush()
        }
    }

    impl Fs for TestFs {
        type Reader = <Dir as Fs>::Reader;
        type Writer = TestWriter;

        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            self.dir.create_dir_all(path)
        }

        fn create_new(&self, path: &Path) -> io::Result<Self::Writer> {
            Ok(TestWriter {
                file: self.dir.create_new(path)?,
                remaining: self.write_limit,
            })
        }

        fn open_read(&self, path: &Path) -> io::Result<Self::Reader> {
            self.dir.open_read(path)
        }

        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            self.pause_before_publication(from, to)?;
            self.dir.rename(from, to)
        }

        fn sync(&self, writer: &mut Self::Writer) -> io::Result<()> {
            self.dir.sync(&mut writer.file)
        }

        fn commit(&self, staging: &Path, final_path: &Path) -> io::Result<Outcome> {
            // Unsupported commits never reach a publication point to pause.
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "redox"
            ))]
            {
                if self.before_rename.is_some() {
                    Dir::validate(staging)?;
                    Dir::validate(final_path)?;
                }
                self.pause_before_publication(staging, final_path)?;
            }
            self.dir.commit(staging, final_path)
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            self.dir.remove_file(path)
        }

        fn metadata(&self, path: &Path) -> io::Result<crate::fs::Kind> {
            self.dir.metadata(path)
        }
    }

    struct CrashChild(Child);

    impl Drop for CrashChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn crash_child(
        root: &Path,
        input: &Path,
        ready: &Path,
        backend_check: bool,
    ) -> io::Result<CrashChild> {
        Command::new(std::env::current_exe()?)
            .args([
                "store::tests::duplicate_put_crash_child",
                "--ignored",
                "--exact",
            ])
            .env("STORE_ROOT", root)
            .env("STORE_INPUT", input)
            .env("STORE_READY", ready)
            .env("STORE_BACKEND_CHECK", if backend_check { "1" } else { "0" })
            .stdout(Stdio::null())
            .spawn()
            .map(CrashChild)
    }

    fn await_candidate(child: &mut CrashChild, ready: &Path) -> io::Result<PathBuf> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.0.try_wait()? {
                return Err(io::Error::other(format!(
                    "child exited before readiness: {status}"
                )));
            }
            match std::fs::read_to_string(ready) {
                Ok(path) if !path.is_empty() => return Ok(PathBuf::from(path)),
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "child did not reach publication",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[ignore = "subprocess entry point; exercised by crash tests"]
    fn duplicate_put_crash_child() {
        let root = PathBuf::from(std::env::var_os("STORE_ROOT").expect("STORE_ROOT"));
        let bytes = std::fs::read(std::env::var_os("STORE_INPUT").expect("STORE_INPUT")).unwrap();
        let mut fs = TestFs::new(Dir::new(&root).unwrap());
        fs.before_rename = Some(PathBuf::from(
            std::env::var_os("STORE_READY").expect("STORE_READY"),
        ));
        if std::env::var("STORE_BACKEND_CHECK").as_deref() == Ok("1") {
            let mut writer = fs.create_new(Path::new("candidate")).unwrap();
            writer.write_all(&bytes).unwrap();
            fs.sync(&mut writer).unwrap();
            drop(writer);
            fs.commit(Path::new("candidate"), Path::new("published"))
                .unwrap();
        } else {
            let store = Store::init_with_fs(fs).unwrap();
            store.put(&bytes).unwrap();
        }
        panic!("paused publication unexpectedly returned");
    }

    #[test]
    fn backend_partial_write_limit_is_cumulative() -> io::Result<()> {
        let tmp = TempDir::new("store")?;
        let mut fs = TestFs::new(Dir::new(tmp.path())?);
        fs.write_limit = Some(3);
        let mut writer = fs.create_new(Path::new("candidate"))?;
        assert_eq!(writer.write(b"")?, 0);
        writer.write_all(b"a")?;
        assert_eq!(writer.write(b"bcde")?, 2);
        assert_eq!(writer.write(b"")?, 0);
        assert!(writer.write_all(b"d").is_err());
        writer.flush()?;
        drop(writer);
        assert_eq!(std::fs::read(tmp.path().join("candidate"))?, b"abc");
        let mut writer = fs.create_new(Path::new("write-all"))?;
        assert!(writer.write_all(b"abcdef").is_err());
        drop(writer);
        assert_eq!(std::fs::read(tmp.path().join("write-all"))?, b"abc");
        fs.write_limit = Some(0);
        let mut writer = fs.create_new(Path::new("zero"))?;
        assert_eq!(writer.write(b"")?, 0);
        assert!(writer.write(b"a").is_err());
        assert_eq!(std::fs::metadata(tmp.path().join("zero"))?.len(), 0);
        Ok(())
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    )))]
    #[test]
    fn unsupported_commit_skips_pause_and_preserves_entries() -> io::Result<()> {
        let tmp = TempDir::new("store")?;
        let signal = TempDir::new("store-signal")?;
        let ready = signal.path().join("absent/ready");
        let mut fs = TestFs::new(Dir::new(&std::path::absolute(tmp.path())?)?);
        fs.before_rename = Some(ready.clone());
        std::fs::write(tmp.path().join("candidate"), b"candidate")?;
        std::fs::write(tmp.path().join("published"), b"published")?;

        let error = match fs.commit(Path::new("candidate"), Path::new("published")) {
            Err(error) => error,
            Ok(_) => panic!("unsupported commit unexpectedly succeeded"),
        };
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(matches!(
            fs.commit(Path::new("candidate"), Path::new("../outside")),
            Err(error) if error.kind() == io::ErrorKind::InvalidInput
        ));
        assert_eq!(std::fs::read(tmp.path().join("candidate"))?, b"candidate");
        assert_eq!(std::fs::read(tmp.path().join("published"))?, b"published");
        assert!(!ready.parent().unwrap().exists());
        Ok(())
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    ))]
    #[test]
    fn backend_pause_precedes_commit_and_kill_leaves_candidate() -> io::Result<()> {
        let tmp = TempDir::new("store")?;
        let signal = TempDir::new("store-signal")?;
        let ready = signal.path().join("ready");
        let input = signal.path().join("input");
        std::fs::write(&input, b"complete")?;
        let mut child = crash_child(tmp.path(), &input, &ready, true)?;
        let candidate = await_candidate(&mut child, &ready)?;
        assert_eq!(candidate, Path::new("candidate"));
        assert_eq!(std::fs::read(tmp.path().join(&candidate))?, b"complete");
        assert!(!tmp.path().join("published").exists());
        child.0.kill()?;
        assert!(!child.0.wait()?.success());
        assert_eq!(std::fs::read(tmp.path().join(&candidate))?, b"complete");
        let fs = TestFs::new(Dir::new(tmp.path())?);
        assert!(matches!(
            fs.commit(&candidate, Path::new("published"))?,
            Outcome::Created
        ));
        assert_eq!(std::fs::read(tmp.path().join("published"))?, b"complete");
        Ok(())
    }

    fn objects(root: &Path) -> color_eyre::Result<Vec<blake3::Hash>> {
        use color_eyre::eyre::bail;
        let mut hashes = Vec::new();
        for shard in std::fs::read_dir(root)? {
            let shard = shard?;
            let prefix = shard.file_name();
            let Some(prefix) = prefix
                .to_str()
                .filter(|name| name.len() == 2 && name.bytes().all(|b| b.is_ascii_hexdigit()))
            else {
                continue;
            };
            if !shard.file_type()?.is_dir() {
                bail!("unexpected non-directory shard: {:?}", shard.path());
            }
            for entry in std::fs::read_dir(shard.path())? {
                let entry = entry?;
                let suffix = entry.file_name();
                let Some(suffix) = suffix
                    .to_str()
                    .filter(|name| name.len() == 62 && name.bytes().all(|b| b.is_ascii_hexdigit()))
                else {
                    continue;
                };
                if !entry.file_type()?.is_file() {
                    bail!("unexpected non-file object: {:?}", entry.path());
                }
                hashes.push(blake3::Hash::from_hex(format!("{prefix}{suffix}"))?);
            }
        }
        Ok(hashes)
    }

    #[test]
    fn object_enumeration_reconstructs_sharded_hashes() -> color_eyre::Result<()> {
        let tmp = TempDir::new("store")?;
        let store = Store {
            dir: TestFs::new(Dir::new(tmp.path())?),
        };
        assert!(objects(tmp.path())?.is_empty());
        Op::GetExisting(0).run(&store, tmp.path())?;
        let expected = [blake3::Hash::from_bytes([0; 32]), blake3::hash(b"object")];
        for hash in expected {
            let path = tmp.path().join(store.object_path(hash));
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, b"object")?;
        }
        std::fs::write(tmp.path().join("input"), b"ignored")?;
        std::fs::write(tmp.path().join("00/candidate"), b"ignored")?;
        let actual = objects(tmp.path())?;
        assert_eq!(actual.len(), expected.len());
        assert!(expected.iter().all(|hash| actual.contains(hash)));
        Ok(())
    }

    /// For modelling arbitrary actions in between our operations of interest
    #[derive(Debug, Arbitrary)]
    enum Op {
        Put(Vec<u8>),
        GetNonexistent(Vec<u8>),
        GetExisting(usize),
        Reopen,
    }

    impl Op {
        pub fn run(&self, store: &Store, root: &Path) -> Result<()> {
            match self {
                Op::Put(bytes) => {
                    store.put(bytes)?;
                }
                Op::GetNonexistent(bytes) => {
                    let hash = blake3::hash(bytes);
                    store.get(hash)?;
                }
                Op::GetExisting(idx) => {
                    let objects = objects(root).unwrap();
                    if objects.is_empty() {
                        return Ok(());
                    }
                    let clamped = idx % objects.len();
                    let key = objects.get(clamped).expect("idx is a valid index");
                    let _ = store.get(*key);
                }
                Op::Reopen => {
                    if let Ok(dir) = Dir::new(root) {
                        let _ = Store::init(dir);
                    }
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
        #[test]
        fn relative_object_path_uses_owned_directory(hash in arb_hash()) {
            use std::io::Read;
            let bytes = *hash.as_bytes();
            let shard = format!("{:02x}", bytes[0]);
            let suffix: String = bytes[1..].iter().map(|byte| format!("{byte:02x}")).collect();
            for (bytes, shard, suffix) in [
                ([0; 32], "00", "0".repeat(62)),
                (
                    std::array::from_fn(|i| i as u8),
                    "00",
                    "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f".to_owned(),
                ),
                ([0xab; 32], "ab", "ab".repeat(31)),
                (bytes, shard.as_str(), suffix),
            ] {
                let tmp = TempDir::new("store")?;
                let root = std::path::absolute(tmp.path())?;
                let store = Store {
                    dir: TestFs::new(Dir::new(&root)?),
                };
                let path = store.object_path(blake3::Hash::from_bytes(bytes));
                let expected = Path::new(shard).join(suffix);
                prop_assert!(path.is_relative());
                prop_assert_eq!(&path, &expected);
                store.dir.create_dir_all(path.parent().unwrap())?;
                let mut writer = store.dir.create_new(&path)?;
                writer.write_all(b"original")?;
                drop(writer);
                prop_assert_eq!(std::fs::read(root.join(&expected))?, b"original");
                std::fs::write(root.join(&expected), b"corrupted")?;
                let mut actual = Vec::new();
                store.dir.open_read(&path)?.read_to_end(&mut actual)?;
                prop_assert_eq!(&actual, b"corrupted");
                drop(store);
                let reopened = Store {
                    dir: TestFs::new(Dir::new(&root)?),
                };
                actual.clear();
                reopened.dir.open_read(&path)?.read_to_end(&mut actual)?;
                prop_assert_eq!(&actual, b"corrupted");
                let unavailable = root.join("unavailable");
                prop_assert_eq!(
                    Dir::new(&unavailable).unwrap_err().kind(),
                    io::ErrorKind::NotFound
                );
                prop_assert!(Op::Reopen.run(&reopened, &unavailable).is_ok());
            }
        }

        /// Self-explanatory, putting some bytes should return its hash
        #[test]
        fn put_returns_hash(bytes in any::<Vec<u8>>()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;

            let key = store.put(&bytes)?;
            let expected = blake3::hash(&bytes);
            prop_assert_eq!(key, expected);
        }

        /// Putting some bytes, then getting it, should return the original
        #[test]
        fn put_get_roundtrips(bytes in any::<Vec<u8>>()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;

            let key = store.put(&bytes)?;
            let obtained = store.get(key)?;
            prop_assert_eq!(obtained, bytes);
        }

        /// Getting a key yields data that hashes to the same key
        #[test]
        fn get_verifies_hash(bytes in any::<Vec<u8>>()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;

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
            let store = Store::init(Dir::new(tmp.path())?)?;
            let obtained = store.get(nonexistent);
            prop_assert!(obtained.is_err());
        }

        /// We obtain the same object that we put in, even after arbitrary
        /// operations
        #[test]
        fn object_id_is_immutable(bytes in any::<Vec<u8>>(), ops in any::<Vec<Op>>()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;
            let key = store.put(&bytes)?;

            for op in ops {
                let _ = op.run(&store, tmp.path());
            }
            drop(store);

            let store = Store::init(Dir::new(tmp.path())?)?;
            let obtained = store.get(key)?;
            prop_assert_eq!(obtained, bytes);
        }
        /// Roundtrip test again, but for boundary sizes in particular for
        /// off-by-one errors
        #[test]
        fn objects_roundtrip_in_boundary(bytes in boundary_sized_bytes()) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;
            let hash = store.put(&bytes).unwrap();
            let actual = store.get(hash).unwrap();
            prop_assert_eq!(actual, bytes);
        }

        /// Putting something multiple times yields the same key
        #[test]
        fn put_idempotence(bytes in any::<Vec<u8>>(), reps in 2usize..1000) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;

            let attempts = (0..reps).map(|_| store.put(&bytes).unwrap()).collect::<Vec<_>>();
            // All attempts must yield the same
            prop_assert!(attempts.windows(2).all(|w| w[0] == w[1]));
        }

        /// Getting something multiple times yields the same key
        #[test]
        fn get_idemptotence(bytes in any::<Vec<u8>>(), reps in 2usize..1000) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;
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
            let store = Store::init(Dir::new(tmp.path())?)?;

            let hash = store.put(&original).unwrap();

            std::fs::write(
                tmp.path().join(store.object_path(hash)),
                &replacement,
            ).unwrap();

            drop(store);
            let store = Store::init(Dir::new(tmp.path())?)?;

            prop_assert!(store.get(hash).is_err());
        }

        /// Reject truncated object files
        #[test]
        fn any_truncation_is_rejected(
            bytes in prop::collection::vec(any::<u8>(), 1..4096),
            numerator in any::<usize>(),
        ) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;

            let hash = store.put(&bytes).unwrap();
            let path = tmp.path().join(store.object_path(hash));

            let new_len = numerator % bytes.len();

            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap();

            file.set_len(new_len as u64).unwrap();

            drop(store);
            let store = Store::init(Dir::new(tmp.path())?)?;

            prop_assert!(store.get(hash).is_err());
        }
        /// Arbitrarily mutate bytes in the file and ensure that `get` rejects
        /// it
        #[test]
        fn arbitrary_byte_mutations_are_rejected(
            (bytes, indices) in corrupted_object(),
        ) {
            let tmp = TempDir::new("test")?;
            let store = Store::init(Dir::new(tmp.path())?)?;

            let hash = store.put(&bytes).unwrap();
            let path = tmp.path().join(store.object_path(hash));

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
            let store = Store::init(Dir::new(tmp.path())?)?;

            let actual_hash = store.put(&x).unwrap();
            prop_assert_eq!(actual_hash, x_hash);

            std::fs::write(
                tmp.path().join(store.object_path(x_hash)),
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
            let store = Store { dir: TestFs::new(Dir::new(tmp.path())?) };

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
            let store = Store { dir: TestFs::new(Dir::new(tmp.path())?) };

            let path = store.object_path(hash);

            let filename = path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();

            prop_assert_eq!(filename.len(), 62);
            let shard = path.parent().unwrap().to_str().unwrap();
            prop_assert_eq!(shard.len(), 2);
            let hex = format!("{shard}{filename}");
            prop_assert_eq!(hex.len(), 64);
            let decoded = blake3::Hash::from_hex(hex).unwrap();

            prop_assert_eq!(decoded, hash);
        }

        #[test]
        fn sharded_path_preserves_full_hash(
            hash in arb_hash(),
        ) {
            let tmp = TempDir::new("test")?;
            let store = Store { dir: TestFs::new(Dir::new(tmp.path())?) };
            let path = store.object_path(hash);

            let relative =
                tmp.path().join(path);

            let relative = relative.strip_prefix(tmp.path()).unwrap();

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
            let mut fs = TestFs::new(Dir::new(tmp.path())?);
            fs.write_limit = Some(prefix_len);
            let store = Store::init_with_fs(fs)?;
            let hash = blake3::hash(&bytes);
            let result = store.put(&bytes);

            prop_assert!(result.is_err());

            // The logical object must not exist.
            prop_assert!(store.get(hash).is_err());
            prop_assert_eq!(
                std::fs::symlink_metadata(tmp.path().join(store.object_path(hash)))
                    .unwrap_err().kind(),
                io::ErrorKind::NotFound,
            );
            drop(store);
            let reopened = Store::init(Dir::new(tmp.path())?)?;
            prop_assert!(reopened.get(hash).is_err());
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
                Store::init(Dir::new(temp.path()).unwrap()).unwrap()
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
                Store::init(Dir::new(temp.path()).unwrap()).unwrap()
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
                Store::init(Dir::new(temp.path()).unwrap()).unwrap()
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
            let signal = TempDir::new("store-signal").unwrap();
            let input = signal.path().join("input");
            let ready = signal.path().join("ready");

            std::fs::write(&input, &bytes).unwrap();

            let mut child = crash_child(&root, &input, &ready, false).unwrap();
            let candidate = await_candidate(&mut child, &ready).unwrap();

            // The first writer now has an abandoned candidate immediately
            // before publication.
            let store = Store::init(Dir::new(&root).unwrap()).unwrap();

            let expected = blake3::hash(&bytes);
            prop_assert_eq!(&std::fs::read(root.join(&candidate)).unwrap(), &bytes);
            prop_assert_eq!(
                std::fs::symlink_metadata(root.join(store.object_path(expected)))
                    .unwrap_err().kind(),
                io::ErrorKind::NotFound,
            );

            // Kill the first writer without cleanup.
            child.0.kill().unwrap();
            prop_assert!(!child.0.wait().unwrap().success());
            prop_assert_eq!(&std::fs::read(root.join(&candidate)).unwrap(), &bytes);

            // A second writer must still be able to establish the object.
            let actual = store.put(&bytes).unwrap();

            prop_assert_eq!(actual, expected);

            drop(store);

            // Verify from a fresh handle, not process-local state.
            let store = Store::init(Dir::new(&root).unwrap()).unwrap();

            prop_assert_eq!(
                store.get(expected).unwrap(),
                bytes,
            );
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Fault {
        Flush,
        Sync,
        Commit,
    }

    impl Fault {
        fn error(self) -> io::Error {
            let kind = match self {
                Self::Flush => io::ErrorKind::BrokenPipe,
                Self::Sync => io::ErrorKind::Other,
                Self::Commit => io::ErrorKind::PermissionDenied,
            };
            io::Error::new(kind, format!("injected {self:?}"))
        }
    }

    #[derive(Default)]
    struct Trace {
        events: Vec<&'static str>,
        owned: Vec<PathBuf>,
        removed: Vec<PathBuf>,
        collision: Option<PathBuf>,
        collide: bool,
        fault: Option<Fault>,
        cleanup_fails: bool,
        open_fails: bool,
    }

    struct CheckFs {
        dir: Dir,
        trace: Arc<Mutex<Trace>>,
        bytes: Vec<u8>,
        resident: bool,
        commit_gate: Option<Mutex<CommitGate>>,
    }

    struct CommitGate {
        phases: mpsc::Sender<&'static str>,
        release: mpsc::Receiver<()>,
    }

    impl CommitGate {
        fn hold(&self, phase: &'static str) -> io::Result<()> {
            self.phases.send(phase).map_err(io::Error::other)?;
            self.release
                .recv_timeout(Duration::from_secs(10))
                .map_err(io::Error::other)
        }
    }

    struct CheckWriter {
        file: File,
        trace: Arc<Mutex<Trace>>,
    }

    impl Write for CheckWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.trace.lock().unwrap().events.push("write");
            self.file.write(bytes)
        }

        fn flush(&mut self) -> io::Result<()> {
            let mut trace = self.trace.lock().unwrap();
            trace.events.push("flush");
            if trace.fault == Some(Fault::Flush) {
                return Err(Fault::Flush.error());
            }
            self.file.flush()
        }
    }

    impl Fs for CheckFs {
        type Reader = File;
        type Writer = CheckWriter;

        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            self.dir.create_dir_all(path)
        }

        fn create_new(&self, path: &Path) -> io::Result<CheckWriter> {
            let hash = blake3::hash(&self.bytes).to_hex();
            let final_path = PathBuf::from(&hash[..2]).join(&hash[2..]);
            assert_ne!(path, final_path);
            let mut trace = self.trace.lock().unwrap();
            if trace.collide {
                trace.collide = false;
                let mut incumbent = self.dir.create_new(path)?;
                incumbent.write_all(b"incumbent sentinel")?;
                trace.collision = Some(path.to_owned());
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, "collision"));
            }
            let file = self.dir.create_new(path)?;
            trace.owned.push(path.to_owned());
            Ok(CheckWriter {
                file,
                trace: self.trace.clone(),
            })
        }

        fn open_read(&self, path: &Path) -> io::Result<File> {
            if self.trace.lock().unwrap().open_fails {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected open",
                ));
            }
            self.dir.open_read(path)
        }

        fn rename(&self, _: &Path, _: &Path) -> io::Result<()> {
            panic!("overwrite rename must never be used")
        }

        fn sync(&self, writer: &mut CheckWriter) -> io::Result<()> {
            let mut trace = self.trace.lock().unwrap();
            assert_eq!(trace.events.last(), Some(&"flush"));
            trace.events.push("sync");
            if trace.fault == Some(Fault::Sync) {
                return Err(Fault::Sync.error());
            }
            self.dir.sync(&mut writer.file)?;
            trace.events.push("synced");
            Ok(())
        }

        fn commit(&self, from: &Path, to: &Path) -> io::Result<Outcome> {
            let mut trace = self.trace.lock().unwrap();
            assert_eq!(trace.events.last(), Some(&"synced"));
            assert_eq!(trace.owned.last().unwrap(), from);
            let mut bytes = Vec::new();
            self.dir.open_read(from)?.read_to_end(&mut bytes)?;
            assert_eq!(bytes, self.bytes);
            if !self.resident {
                assert_eq!(
                    self.dir.metadata(to).err().unwrap().kind(),
                    io::ErrorKind::NotFound
                );
            }
            trace.events.push("commit");
            if trace.fault == Some(Fault::Commit) {
                return Err(Fault::Commit.error());
            }
            drop(trace);
            if let Some(gate) = &self.commit_gate {
                gate.lock().unwrap().hold("before")?;
            }
            let outcome = self.dir.commit(from, to)?;
            if let Some(gate) = &self.commit_gate {
                assert!(matches!(outcome, Outcome::Created));
                gate.lock().unwrap().hold("after")?;
            }
            Ok(outcome)
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            let mut trace = self.trace.lock().unwrap();
            assert!(trace.owned.iter().any(|owned| owned == path));
            assert_ne!(trace.collision.as_deref(), Some(path));
            trace.removed.push(path.to_owned());
            if trace.cleanup_fails {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected cleanup",
                ));
            }
            self.dir.remove_file(path)
        }

        fn metadata(&self, path: &Path) -> io::Result<crate::fs::Kind> {
            self.dir.metadata(path)
        }
    }

    fn backend(root: &Path, bytes: &[u8], trace: Trace, resident: bool) -> Backend<CheckFs> {
        Backend::init_with_fs(CheckFs {
            dir: Dir::new(root).unwrap(),
            trace: Arc::new(Mutex::new(trace)),
            bytes: bytes.to_vec(),
            resident,
            commit_gate: None,
        })
        .unwrap()
    }

    fn assert_put_error(result: Result<blake3::Hash>, bytes: &[u8], fault: Fault) {
        match result {
            Err(Error::PutError { hash, error }) => {
                assert_eq!(hash, blake3::hash(bytes));
                assert_eq!(error.kind(), fault.error().kind());
                assert_eq!(error.to_string(), fault.error().to_string());
            }
            other => panic!("expected primary {fault:?} error, got {other:?}"),
        }
    }

    // partial writes and retries need separate expectations
    const NONEMPTY_PUT_EVENTS: &[&str] = &["write", "flush", "sync", "synced", "commit"];

    #[test]
    fn collision_and_publication_boundaries() {
        for bytes in [b"complete bytes".as_slice(), b"".as_slice()] {
            let tmp = TempDir::new("store-check").unwrap();
            let store = backend(
                tmp.path(),
                bytes,
                Trace {
                    collide: true,
                    ..Trace::default()
                },
                false,
            );
            let hash = store.put(bytes).unwrap();
            assert_eq!(store.get(hash).unwrap(), bytes);
            let trace = store.dir.trace.lock().unwrap();
            assert_eq!(trace.owned.len(), 1);
            assert!(trace.removed.is_empty());
            assert!(!tmp.path().join(&trace.owned[0]).exists());
            assert_eq!(
                std::fs::read(tmp.path().join(trace.collision.as_ref().unwrap())).unwrap(),
                b"incumbent sentinel"
            );
            let expected = if bytes.is_empty() {
                &NONEMPTY_PUT_EVENTS[1..] // Empty input omits write.
            } else {
                NONEMPTY_PUT_EVENTS
            };
            assert_eq!(trace.events, expected);
        }
    }

    #[test]
    fn publication_failures_preserve_primary_error_and_cleanup_ownership() {
        for fault in [Fault::Flush, Fault::Sync, Fault::Commit] {
            let tmp = TempDir::new("store-check").unwrap();
            let bytes = b"complete bytes";
            let store = backend(
                tmp.path(),
                bytes,
                Trace {
                    fault: Some(fault),
                    collide: true,
                    ..Trace::default()
                },
                false,
            );
            assert_put_error(store.put(bytes), bytes, fault);
            assert!(!tmp
                .path()
                .join(store.object_path(blake3::hash(bytes)))
                .exists());
            let trace = store.dir.trace.lock().unwrap();
            assert_eq!(trace.owned.len(), 1);
            assert_eq!(trace.removed, trace.owned);
            assert!(!tmp.path().join(&trace.owned[0]).exists());
            assert_eq!(
                std::fs::read(tmp.path().join(trace.collision.as_ref().unwrap())).unwrap(),
                b"incumbent sentinel"
            );
            let endpoint = match fault {
                Fault::Flush => "flush",
                Fault::Sync => "sync",
                Fault::Commit => "commit",
            };
            let end = NONEMPTY_PUT_EVENTS
                .iter()
                .position(|&event| event == endpoint)
                .expect("fault endpoint missing from expected put ordering");
            let expected = &NONEMPTY_PUT_EVENTS[..=end];
            assert_eq!(trace.events, expected);
        }
    }

    #[test]
    fn failed_cleanup_leaves_an_unpoisoning_orphan() {
        let tmp = TempDir::new("store-check").unwrap();
        let bytes = b"orphan bytes";
        let store = backend(
            tmp.path(),
            bytes,
            Trace {
                fault: Some(Fault::Sync),
                cleanup_fails: true,
                ..Trace::default()
            },
            false,
        );
        assert_put_error(store.put(bytes), bytes, Fault::Sync);
        assert!(!tmp
            .path()
            .join(store.object_path(blake3::hash(bytes)))
            .exists());
        let orphan = {
            let trace = store.dir.trace.lock().unwrap();
            assert_eq!(trace.owned.len(), 1);
            assert_eq!(trace.removed, trace.owned);
            tmp.path().join(&trace.owned[0])
        };
        assert_eq!(std::fs::read(&orphan).unwrap(), bytes);
        let later = backend(tmp.path(), bytes, Trace::default(), false);
        let hash = later.put(bytes).unwrap();
        assert_eq!(later.get(hash).unwrap(), bytes);
        assert_eq!(std::fs::read(&orphan).unwrap(), bytes);
        let trace = later.dir.trace.lock().unwrap();
        assert!(trace.removed.is_empty());
        assert!(!tmp.path().join(&trace.owned[0]).exists());
    }

    #[test]
    fn duplicate_put_verifies_resident_without_repair() {
        for corrupt in [false, true] {
            let tmp = TempDir::new("store-check").unwrap();
            let bytes = b"original object";
            let store = backend(tmp.path(), bytes, Trace::default(), true);
            let hash = blake3::hash(bytes);
            let path = tmp.path().join(store.object_path(hash));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let resident = if corrupt {
                b"corrupt".as_slice()
            } else {
                bytes
            };
            std::fs::write(&path, resident).unwrap();
            let other_bytes = b"another object";
            let other_hash = blake3::hash(other_bytes);
            let other_path = tmp.path().join(store.object_path(other_hash));
            std::fs::create_dir_all(other_path.parent().unwrap()).unwrap();
            std::fs::write(&other_path, other_bytes).unwrap();
            if corrupt {
                match store.put(bytes) {
                    Err(Error::PutError { hash: got, error }) => {
                        assert_eq!(got, hash);
                        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
                    }
                    other => panic!("corrupt duplicate accepted: {other:?}"),
                }
                assert_get_error(store.get(hash), hash, io::ErrorKind::InvalidData, None);
            } else {
                assert_eq!(store.put(bytes).unwrap(), hash);
                assert_eq!(store.get(hash).unwrap(), bytes);
            }
            assert_eq!(std::fs::read(path).unwrap(), resident);
            assert_eq!(store.get(other_hash).unwrap(), other_bytes);
            let trace = store.dir.trace.lock().unwrap();
            assert_eq!(trace.owned.len(), 1);
            assert_eq!(trace.removed, trace.owned);
            assert!(!tmp.path().join(&trace.owned[0]).exists());
        }
    }

    fn assert_get_error(
        result: Result<Vec<u8>>,
        key: blake3::Hash,
        kind: io::ErrorKind,
        message: Option<&str>,
    ) {
        match result {
            Err(Error::GetError { hash, error }) => {
                assert_eq!(hash, key);
                assert_eq!(error.kind(), kind);
                if let Some(message) = message {
                    assert_eq!(error.to_string(), message);
                }
            }
            other => panic!("expected GetError, got {other:?}"),
        }
    }

    #[test]
    fn get_distinguishes_missing_from_original_io_error() {
        let tmp = TempDir::new("store-check").unwrap();
        let bytes = b"missing object";
        let store = backend(tmp.path(), bytes, Trace::default(), false);
        let key = blake3::hash(bytes);
        match store.get(key) {
            Err(Error::PathNotFound { path }) => assert_eq!(path, store.object_path(key)),
            other => panic!("expected PathNotFound, got {other:?}"),
        }
        store.dir.trace.lock().unwrap().open_fails = true;
        assert_get_error(
            store.get(key),
            key,
            io::ErrorKind::PermissionDenied,
            Some("injected open"),
        );
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "redox"))]
    #[test]
    fn separate_reader_gets_at_both_commit_boundaries() {
        let tmp = TempDir::new("store-check").unwrap();
        let bytes = b"complete object at both publication boundaries";
        let hash = blake3::hash(bytes);
        let reader = super::Store::init(Dir::new(tmp.path()).unwrap()).unwrap();
        let mut writer = backend(tmp.path(), bytes, Trace::default(), false);
        let path = writer.object_path(hash);

        std::thread::scope(|scope| {
            // These endpoints drop before scope joins the worker on assertion failure.
            let (phases_tx, phases_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            writer.dir.commit_gate = Some(Mutex::new(CommitGate {
                phases: phases_tx,
                release: release_rx,
            }));
            let worker = scope.spawn(move || writer.put(bytes));
            let mut observed = Vec::new();
            for expected in ["before", "after"] {
                let phase = phases_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                assert_eq!(phase, expected);
                assert!(!worker.is_finished(), "put returned before {phase} read");
                match phase {
                    "before" => match reader.get(hash) {
                        Err(Error::PathNotFound { path: actual }) => assert_eq!(actual, path),
                        other => panic!("object exposed before commit: {other:?}"),
                    },
                    "after" => {
                        let actual = reader.get(hash).unwrap();
                        assert_eq!(actual, bytes);
                        assert_eq!(blake3::hash(&actual), hash);
                    }
                    _ => unreachable!(),
                }
                assert!(!worker.is_finished(), "put returned during {phase} read");
                observed.push(phase);
                release_tx.send(()).unwrap();
            }
            assert_eq!(observed, ["before", "after"]);
            assert_eq!(worker.join().unwrap().unwrap(), hash);
            assert_eq!(phases_rx.try_recv(), Err(mpsc::TryRecvError::Disconnected));
        });
    }
