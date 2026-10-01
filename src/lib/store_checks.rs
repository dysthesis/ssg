use super::*;
use std::{
    fs::File,
    path::Path,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
use tempdir::TempDir;

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
    let reader = Store::init(Dir::new(tmp.path()).unwrap()).unwrap();
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
