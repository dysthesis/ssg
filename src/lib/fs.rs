use cap_std::ambient_authority;
use cap_std::fs::{Dir as CapDir, OpenOptions};

use std::{
    fs::File,
    io::{self, Read, Result, Write},
    path::{Component, Path},
};

/// Filesystem interface for swappable backends, e.g. fault injection.
pub(crate) trait Fs: Send + Sync {
    type Reader: Read;
    type Writer: Write;

    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn create_new(&self, path: &Path) -> Result<Self::Writer>;
    fn open_read(&self, path: &Path) -> Result<Self::Reader>;
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;
    /// Synchronise the writer's file data and metadata to storage. This does
    /// not synchronise the containing directory entry.
    fn sync(&self, file: &mut Self::Writer) -> Result<()>;
    /// Atomically move `staging` to `final_path` without replacing any existing
    /// destination entry. On [`Outcome::Existing`], both entries are unchanged;
    /// on [`Outcome::Created`], the staging entry is moved.
    /// Returns `Unsupported` where no sufficient atomic primitive exists.
    fn commit(&self, staging: &Path, final_path: &Path) -> Result<Outcome>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<Kind>;
}

/// The outcome of [`Fs::commit`]
pub(crate) enum Outcome {
    /// The staging entry was moved to the destination.
    Created,
    /// A destination entry already existed.
    Existing,
}

/// The kind of a given filesystem object. For now, we only care about files and
/// directories.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    File,
    Directory,
    Other,
}

/// Filesystem operations rooted at an opened directory capability.
///
/// Operation paths must have one or more normal components after ignoring
/// current-directory components (`.`). Parent, root, and prefix components
/// are rejected before filesystem I/O.
#[derive(Debug)]
pub struct Dir {
    cap_root: CapDir,
}

impl Dir {
    /// Open an existing absolute directory as a filesystem capability.
    /// Later operations use this opened directory, not its supplied pathname.
    pub fn new(root: &Path) -> io::Result<Self> {
        if !root.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "root must be an absolute directory",
            ));
        }

        let cap_root = CapDir::open_ambient_dir(root, ambient_authority())?;
        Ok(Self { cap_root })
    }

    #[inline]
    pub(crate) fn validate(path: &Path) -> io::Result<()> {
        let mut components = path
            .components()
            .filter(|component| *component != Component::CurDir);
        if matches!(components.next(), Some(Component::Normal(_)))
            && components.all(|component| matches!(component, Component::Normal(_)))
        {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected a nonempty relative path without parent components",
            ))
        }
    }
}

impl Fs for Dir {
    type Reader = File;

    type Writer = File;

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        Self::validate(path)?;
        self.cap_root.create_dir_all(path)
    }

    fn create_new(&self, path: &Path) -> Result<Self::Writer> {
        Self::validate(path)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        self.cap_root
            .open_with(path, &options)
            .map(|file| file.into_std())
    }

    fn open_read(&self, path: &Path) -> Result<Self::Reader> {
        Self::validate(path)?;
        self.cap_root.open(path).map(|file| file.into_std())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        Self::validate(from)?;
        Self::validate(to)?;
        self.cap_root.rename(from, &self.cap_root, to)
    }

    fn sync(&self, file: &mut Self::Writer) -> Result<()> {
        #[cfg(target_vendor = "apple")]
        {
            rustix::fs::fcntl_fullfsync(&*file).map_err(Into::into)
        }
        #[cfg(all(not(windows), not(target_vendor = "apple")))]
        {
            rustix::fs::fsync(&*file).map_err(Into::into)
        }
        #[cfg(windows)]
        {
            file.sync_all()
        }
    }

    fn commit(&self, staging: &Path, final_path: &Path) -> Result<Outcome> {
        Self::validate(staging)?;
        Self::validate(final_path)?;

        #[cfg(any(target_os = "linux", target_os = "android", target_os = "redox"))]
        {
            use rustix::fs::{RenameFlags, renameat_with};

            // Return the parent, the raw source leaf, and the destination leaf with
            // trailing slashes removed, matching cap-std's rename path handling.
            fn split_commit_path(path: &Path) -> (&Path, &std::ffi::OsStr, &std::ffi::OsStr) {
                use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

                let bytes = path.as_os_str().as_bytes();
                let end = bytes
                    .iter()
                    .rposition(|&byte| byte != b'/')
                    .map_or(0, |index| index + 1);
                let start = bytes[..end]
                    .iter()
                    .rposition(|&byte| byte == b'/')
                    .map_or(0, |index| index + 1);
                (
                    Path::new(OsStr::from_bytes(&bytes[..start])),
                    OsStr::from_bytes(&bytes[start..]),
                    OsStr::from_bytes(&bytes[start..end]),
                )
            }

            let (from_parent, from_leaf, _) = split_commit_path(staging);
            let (to_parent, _, to_leaf) = split_commit_path(final_path);
            let from_dir = if from_parent
                .components()
                .all(|component| component == Component::CurDir)
            {
                None
            } else {
                Some(self.cap_root.open_dir(from_parent)?)
            };

            let to_dir = if to_parent == from_parent
                || to_parent
                    .components()
                    .all(|component| component == Component::CurDir)
            {
                None
            } else {
                Some(self.cap_root.open_dir(to_parent)?)
            };
            let from_dir = from_dir.as_ref().unwrap_or(&self.cap_root);
            let to_dir = if to_parent == from_parent {
                from_dir
            } else {
                to_dir.as_ref().unwrap_or(&self.cap_root)
            };

            return match renameat_with(from_dir, from_leaf, to_dir, to_leaf, RenameFlags::NOREPLACE)
            {
                Ok(()) => Ok(Outcome::Created),
                Err(rustix::io::Errno::EXIST) => Ok(Outcome::Existing),
                Err(error) => Err(error.into()),
            };
        }
        #[allow(unreachable_code)] // Supported targets return above.
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "atomic no-replace semantics are unavailable on this platform",
            ))
        }
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        Self::validate(path)?;
        self.cap_root.remove_file(path)
    }

    fn metadata(&self, path: &Path) -> Result<Kind> {
        Self::validate(path)?;
        let kind = self.cap_root.symlink_metadata(path)?.file_type();
        Ok(if kind.is_file() {
            Kind::File
        } else if kind.is_dir() {
            Kind::Directory
        } else {
            Kind::Other
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;
    use tempdir::TempDir;
    fn relative_path() -> impl Strategy<Value = PathBuf> {
        prop::collection::vec(component(), 1..=4).prop_map(|parts| {
            let mut path = PathBuf::new();

            for part in parts {
                path.push(part);
            }

            path
        })
    }
    fn parent_path() -> impl Strategy<Value = PathBuf> {
        (
            prop::collection::vec(component(), 0..=2),
            prop::collection::vec(component(), 0..=2),
        )
            .prop_map(|(before, after)| {
                let mut path = PathBuf::new();

                for part in before {
                    path.push(part);
                }

                path.push("..");

                for part in after {
                    path.push(part);
                }

                path
            })
    }
    fn tempdir_name() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("a".to_owned()),
            Just("dir".to_owned()),
            proptest::string::string_regex("[a-zA-Z0-9_-]{1,64}").unwrap(),
        ]
    }

    fn component() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-z0-9][a-z0-9_-]{0,31}").unwrap()
    }

    /// We want to make sure that dir traversal is prevented by _our mechanism_
    /// and not some unrelated OS error
    fn assert_invalid_input<T>(result: io::Result<T>) {
        match result {
            Err(error) => {
                assert_eq!(
                    error.kind(),
                    io::ErrorKind::InvalidInput,
                    "expected InvalidInput, got {error:?}",
                );
            }

            Ok(_) => {
                panic!("operation unexpectedly accepted an invalid path");
            }
        }
    }

    // Drop the opened capability before tempdir cleanup; verify preservation
    // through std::fs paths, not the capability under test.
    struct EscapeSandbox {
        dir: Dir,
        root: PathBuf,
        inside: PathBuf,
        _tmp: TempDir,
    }

    impl EscapeSandbox {
        fn new(bytes: &[u8]) -> io::Result<Self> {
            let tmp = TempDir::new("dir")?;
            let parent = std::path::absolute(tmp.path())?;
            let root = parent.join("root");
            std::fs::create_dir(&root)?;
            let inside = root.join("inside");
            std::fs::write(&inside, bytes)?;
            let dir = Dir::new(&root)?;
            Ok(Self {
                dir,
                root,
                inside,
                _tmp: tmp,
            })
        }

        fn assert_inside_unchanged(&self, bytes: &[u8]) {
            assert!(self.root.is_dir());
            assert_eq!(std::fs::read(&self.inside).unwrap().as_slice(), bytes);
        }
    }

    #[cfg(unix)]
    struct SymlinkSandbox {
        dir: Dir,
        root: PathBuf,
        outside: PathBuf,
        link_target: PathBuf,
        _tmp: TempDir,
    }

    #[cfg(unix)]
    impl SymlinkSandbox {
        fn new(absolute_link: bool) -> io::Result<Self> {
            use std::os::unix::fs::symlink;

            let tmp = TempDir::new("dir")?;
            let parent = std::path::absolute(tmp.path())?;
            let root = parent.join("root");
            let outside = parent.join("outside");
            std::fs::create_dir(&root)?;
            std::fs::create_dir(&outside)?;
            std::fs::create_dir(root.join("inside"))?;
            std::fs::write(root.join("inside/sentinel"), b"inside sentinel")?;
            std::fs::write(outside.join("sentinel"), b"outside sentinel")?;
            let link_target = if absolute_link {
                outside.clone()
            } else {
                PathBuf::from("../outside")
            };
            symlink(&link_target, root.join("escape"))?;
            let dir = Dir::new(&root)?;
            Ok(Self {
                dir,
                root,
                outside,
                link_target,
                _tmp: tmp,
            })
        }

        fn assert_unchanged(&self) -> io::Result<()> {
            assert!(self.root.is_dir());
            assert_eq!(
                std::fs::read(self.root.join("inside/sentinel"))?,
                b"inside sentinel"
            );
            assert_eq!(
                std::fs::read(self.outside.join("sentinel"))?,
                b"outside sentinel"
            );
            assert!(std::fs::symlink_metadata(self.root.join("escape"))?
                .file_type()
                .is_symlink());
            assert_eq!(
                std::fs::read_link(self.root.join("escape"))?,
                self.link_target
            );
            let inside_names = std::fs::read_dir(self.root.join("inside"))?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<io::Result<Vec<_>>>()?;
            let outside_names = std::fs::read_dir(&self.outside)?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<io::Result<Vec<_>>>()?;
            assert_eq!(inside_names, vec![std::ffi::OsString::from("sentinel")]);
            assert_eq!(outside_names, vec![std::ffi::OsString::from("sentinel")]);
            Ok(())
        }
    }

    #[cfg(unix)]
    fn assert_absent(path: &Path) {
        assert_eq!(
            std::fs::symlink_metadata(path).unwrap_err().kind(),
            io::ErrorKind::NotFound,
            "unexpected object at {path:?}",
        );
    }

    #[test]
    fn empty_path_is_rejected() -> color_eyre::Result<()> {
        let tmp = TempDir::new("dir")?;
        let root = std::path::absolute(tmp.path())?;
        let dir = Dir::new(&root)?;

        assert_invalid_input(dir.create_new(Path::new("")));
        assert!(std::fs::read_dir(&root)?.next().is_none());

        Ok(())
    }

    #[test]
    fn current_directory_path_is_rejected() -> color_eyre::Result<()> {
        let tmp = TempDir::new("dir")?;
        let root = std::path::absolute(tmp.path())?;
        std::fs::write(root.join("sentinel"), b"intact")?;
        let dir = Dir::new(&root)?;

        assert_invalid_input(dir.remove_file(Path::new(".")));
        assert_eq!(std::fs::read(root.join("sentinel"))?, b"intact");

        Ok(())
    }

    #[test]
    fn current_directory_components_work_in_operations() -> color_eyre::Result<()> {
        let tmp = TempDir::new("dir")?;
        let dir = Dir::new(&std::path::absolute(tmp.path())?)?;

        dir.create_dir_all(Path::new("./nested/./child"))?;
        let mut writer = dir.create_new(Path::new("./nested/./child/file"))?;
        writer.write_all(b"content")?;
        drop(writer);

        dir.rename(
            Path::new("./nested/./child/file"),
            Path::new("nested/./child/moved"),
        )?;
        let mut actual = Vec::new();
        dir.open_read(Path::new("./nested/./child/moved"))?
            .read_to_end(&mut actual)?;
        assert_eq!(actual, b"content");
        assert!(dir.metadata(Path::new("nested/./child/moved"))? == Kind::File);
        dir.remove_file(Path::new("./nested/./child/moved"))?;
        assert_eq!(
            dir.open_read(Path::new("nested/child/moved"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );

        Ok(())
    }

    proptest! {
        #[test]
        fn existing_absolute_directory_is_accepted(
            name in tempdir_name(),
        ) {
            let tmp = TempDir::new(&name)?;

            let root = std::path::absolute(tmp.path())?;
            prop_assert!(root.is_absolute());
            prop_assert!(root.is_dir());

            std::fs::write(root.join("probe"), b"rooted")?;
            let dir = Dir::new(&root)?;
            let mut actual = Vec::new();
            dir.open_read(Path::new("probe"))?.read_to_end(&mut actual)?;
            prop_assert_eq!(actual.as_slice(), b"rooted");
        }

        #[test]
        fn nonexistent_relative_root_is_rejected(
            root in relative_path(),
        ) {
            prop_assume!(!root.exists());

            let error = Dir::new(&root).unwrap_err();

            prop_assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
            );
        }

        #[test]
        fn existing_relative_root_is_rejected(
            name in tempdir_name(),
        ) {
            let cwd = std::env::current_dir().unwrap();

            let tmp = TempDir::new_in(&cwd, &name).unwrap();

            let relative = tmp
                .path()
                .strip_prefix(&cwd)
                .unwrap()
                .to_path_buf();

            prop_assert!(relative.is_relative());
            prop_assert!(relative.is_dir());

            let error = Dir::new(&relative).unwrap_err();

            prop_assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
            );
        }

        #[test]
        fn missing_root_is_rejected(
            name in component(),
        ) {
            let tmp = tempdir::TempDir::new("dir").unwrap();

            let root = std::path::absolute(
                tmp.path().join(name)
            )
            .unwrap();

            prop_assert!(!root.exists());
            prop_assert!(Dir::new(&root).is_err());
        }

        #[test]
        fn file_cannot_be_root(
            bytes in any::<Vec<u8>>(),
        ) {
            let tmp = tempdir::TempDir::new("dir").unwrap();

            let root = std::path::absolute(
                tmp.path().join("root")
            )
            .unwrap();

            std::fs::write(&root, &bytes).unwrap();

            prop_assert!(Dir::new(&root).is_err());

            prop_assert_eq!(
                std::fs::read(root).unwrap(),
                bytes
            );
        }

        // Dir traversal prevention tests
        #[test]
        fn parent_components_are_rejected(
            path in parent_path(),
        ) {
            let tmp = tempdir::TempDir::new("dir").unwrap();
            let root = std::path::absolute(tmp.path()).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, b"protected").unwrap();
            let dir = Dir::new(&root).unwrap();

            assert_invalid_input(dir.rename(Path::new("inside"), &path));
            prop_assert_eq!(std::fs::read(&inside).unwrap(), b"protected");
        }

        #[test]
        fn absolute_paths_are_rejected(
            relative in relative_path(),
        ) {
            let tmp = tempdir::TempDir::new("dir").unwrap();
            let root = std::path::absolute(tmp.path()).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, b"protected").unwrap();
            let dir = Dir::new(&root).unwrap();
            let absolute = root.join(relative);

            assert_invalid_input(dir.rename(Path::new("inside"), &absolute));
            prop_assert_eq!(std::fs::read(&inside).unwrap(), b"protected");
        }

        #[test]
        fn open_read_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let sandbox = EscapeSandbox::new(&bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = sandbox.root.parent().unwrap().join(&sibling);
            std::fs::write(&outside, &bytes).unwrap();

            assert_invalid_input(sandbox.dir.open_read(&Path::new("..").join(&sibling)));

            sandbox.assert_inside_unchanged(&bytes);
            prop_assert_eq!(std::fs::read(&outside).unwrap(), bytes);
        }

        #[test]
        fn metadata_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let sandbox = EscapeSandbox::new(&bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = sandbox.root.parent().unwrap().join(&sibling);
            std::fs::write(&outside, &bytes).unwrap();

            assert_invalid_input(sandbox.dir.metadata(&Path::new("..").join(&sibling)));

            sandbox.assert_inside_unchanged(&bytes);
            prop_assert_eq!(std::fs::read(&outside).unwrap(), bytes);
        }

        #[test]
        fn remove_file_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let sandbox = EscapeSandbox::new(&bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = sandbox.root.parent().unwrap().join(&sibling);
            std::fs::write(&outside, &bytes).unwrap();

            assert_invalid_input(sandbox.dir.remove_file(&Path::new("..").join(&sibling)));

            sandbox.assert_inside_unchanged(&bytes);
            prop_assert_eq!(std::fs::read(&outside).unwrap(), bytes);
        }

        #[test]
        fn create_new_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let sandbox = EscapeSandbox::new(&bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = sandbox.root.parent().unwrap().join(&sibling);
            prop_assert!(!outside.exists());

            assert_invalid_input(sandbox.dir.create_new(&Path::new("..").join(&sibling)));

            sandbox.assert_inside_unchanged(&bytes);
            prop_assert!(!outside.exists());
        }

        #[test]
        fn create_dir_all_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let sandbox = EscapeSandbox::new(&bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = sandbox.root.parent().unwrap().join(&sibling);
            prop_assert!(!outside.exists());

            assert_invalid_input(sandbox.dir.create_dir_all(&Path::new("..").join(&sibling)));

            sandbox.assert_inside_unchanged(&bytes);
            prop_assert!(!outside.exists());
        }

        #[test]
        fn rename_source_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let sandbox = EscapeSandbox::new(&bytes).unwrap();
            let destination = sandbox.root.join("destination");
            prop_assert!(!destination.exists());
            let sibling = format!("outside-{name}");
            let outside = sandbox.root.parent().unwrap().join(&sibling);
            std::fs::write(&outside, &bytes).unwrap();

            assert_invalid_input(sandbox.dir.rename(
                &Path::new("..").join(&sibling),
                Path::new("destination"),
            ));

            sandbox.assert_inside_unchanged(&bytes);
            prop_assert_eq!(std::fs::read(&outside).unwrap(), bytes);
            prop_assert!(!destination.exists());
        }

        #[test]
        fn rename_destination_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let sandbox = EscapeSandbox::new(&bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = sandbox.root.parent().unwrap().join(&sibling);
            prop_assert!(!outside.exists());

            assert_invalid_input(sandbox.dir.rename(
                Path::new("inside"),
                &Path::new("..").join(&sibling),
            ));

            sandbox.assert_inside_unchanged(&bytes);
            prop_assert!(!outside.exists());
        }
    }

    proptest! {
        #[test]
        fn create_dir_all_creates_every_component(path in relative_path()) {
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;

            dir.create_dir_all(&path)?;

            let mut prefix = PathBuf::new();
            for component in path.components() {
                prefix.push(component);
                prop_assert!(dir.metadata(&prefix)? == Kind::Directory);
            }
        }

        #[test]
        fn create_dir_all_repeatedly_converges(path in relative_path()) {
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;

            dir.create_dir_all(&path)?;
            dir.create_dir_all(&path)?;

            let mut prefix = PathBuf::new();
            for component in path.components() {
                prefix.push(component);
                prop_assert!(dir.metadata(&prefix)? == Kind::Directory);
            }
        }

        #[test]
        fn concurrent_create_dir_all_converges(
            path in relative_path(),
            workers in 2usize..=8,
        ) {
            use std::sync::{Arc, Barrier};
            use std::thread;

            let tmp = TempDir::new("dir")?;
            let dir = Arc::new(Dir::new(&std::path::absolute(tmp.path())?)?);
            let barrier = Arc::new(Barrier::new(workers + 1));
            let handles: Vec<_> = (0..workers).map(|_| {
                let dir = Arc::clone(&dir);
                let barrier = Arc::clone(&barrier);
                let path = path.clone();
                thread::spawn(move || {
                    barrier.wait();
                    dir.create_dir_all(&path)
                })
            }).collect();

            barrier.wait();
            let results: Vec<_> = handles.into_iter().map(|handle| handle.join()).collect();
            for result in results {
                result.expect("worker panicked")?;
            }
            prop_assert!(dir.metadata(&path)? == Kind::Directory);
        }

        #[test]
        fn create_new_writer_roundtrips(name in component(), bytes in any::<Vec<u8>>()) {
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;
            let path = Path::new(&name);

            let mut writer = dir.create_new(path)?;
            writer.write_all(&bytes)?;
            drop(writer);

            let mut actual = Vec::new();
            dir.open_read(path)?.read_to_end(&mut actual)?;
            prop_assert_eq!(actual, bytes);
            prop_assert!(dir.metadata(path)? == Kind::File);
        }

        #[test]
        fn create_new_never_overwrites_existing_file(
            name in component(),
            bytes in any::<Vec<u8>>(),
        ) {
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;
            let path = Path::new(&name);

            let mut writer = dir.create_new(path)?;
            writer.write_all(&bytes)?;
            drop(writer);

            prop_assert!(dir.create_new(path).is_err());
            let mut actual = Vec::new();
            dir.open_read(path)?.read_to_end(&mut actual)?;
            prop_assert_eq!(actual, bytes);
        }

        #[test]
        fn concurrent_create_new_has_exactly_one_winner(
            name in component(),
            workers in 2usize..=16,
        ) {
            use std::sync::{Arc, Barrier};
            use std::thread;

            let tmp = TempDir::new("dir")?;
            let dir = Arc::new(Dir::new(&std::path::absolute(tmp.path())?)?);
            let barrier = Arc::new(Barrier::new(workers + 1));
            let handles: Vec<_> = (0..workers).map(|_| {
                let dir = Arc::clone(&dir);
                let barrier = Arc::clone(&barrier);
                let name = name.clone();
                thread::spawn(move || {
                    barrier.wait();
                    dir.create_new(Path::new(&name))
                })
            }).collect();

            barrier.wait();
            let results: Vec<_> = handles.into_iter().map(|handle| handle.join()).collect();
            let winners = results.into_iter()
                .map(|result| result.expect("worker panicked"))
                .filter(Result::is_ok)
                .count();
            prop_assert_eq!(winners, 1);
            prop_assert!(dir.metadata(Path::new(&name))? == Kind::File);
        }

        #[test]
        fn open_read_preserves_not_found(path in relative_path()) {
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;

            prop_assert_eq!(dir.open_read(&path).unwrap_err().kind(), io::ErrorKind::NotFound);
        }

        #[test]
        fn rename_moves_file_without_changing_bytes(
            from in component(), to in component(), bytes in any::<Vec<u8>>(),
        ) {
            prop_assume!(from != to);
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;
            let from = Path::new(&from);
            let to = Path::new(&to);
            let mut writer = dir.create_new(from)?;
            writer.write_all(&bytes)?;
            drop(writer);

            dir.rename(from, to)?;
            prop_assert_eq!(dir.open_read(from).unwrap_err().kind(), io::ErrorKind::NotFound);
            let mut actual = Vec::new();
            dir.open_read(to)?.read_to_end(&mut actual)?;
            prop_assert_eq!(actual, bytes);
        }

        #[test]
        fn rename_replaces_existing_file(
            from in component(), to in component(),
            bytes in any::<Vec<u8>>(), old_destination in any::<Vec<u8>>(),
        ) {
            prop_assume!(from != to);
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;
            let from = Path::new(&from);
            let to = Path::new(&to);
            let mut source = dir.create_new(from)?;
            source.write_all(&bytes)?;
            drop(source);
            let mut destination = dir.create_new(to)?;
            destination.write_all(&old_destination)?;
            drop(destination);

            dir.rename(from, to)?;
            prop_assert_eq!(dir.open_read(from).unwrap_err().kind(), io::ErrorKind::NotFound);
            let mut actual = Vec::new();
            dir.open_read(to)?.read_to_end(&mut actual)?;
            prop_assert_eq!(actual, bytes);
        }

        #[test]
        fn remove_file_removes_only_target(
            target in component(), sibling in component(),
            bytes in any::<Vec<u8>>(), sibling_bytes in any::<Vec<u8>>(),
        ) {
            prop_assume!(target != sibling);
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;
            let target = Path::new(&target);
            let sibling = Path::new(&sibling);
            let mut writer = dir.create_new(target)?;
            writer.write_all(&bytes)?;
            drop(writer);
            let mut writer = dir.create_new(sibling)?;
            writer.write_all(&sibling_bytes)?;
            drop(writer);

            dir.remove_file(target)?;
            prop_assert_eq!(dir.open_read(target).unwrap_err().kind(), io::ErrorKind::NotFound);
            let mut actual = Vec::new();
            dir.open_read(sibling)?.read_to_end(&mut actual)?;
            prop_assert_eq!(actual, sibling_bytes);
        }

        #[test]
        fn kind_distinguishes_file_and_directory(file in component(), directory in component()) {
            prop_assume!(file != directory);
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(&std::path::absolute(tmp.path())?)?;
            drop(dir.create_new(Path::new(&file))?);
            dir.create_dir_all(Path::new(&directory))?;

            prop_assert!(dir.metadata(Path::new(&file))? == Kind::File);
            prop_assert!(dir.metadata(Path::new(&directory))? == Kind::Directory);
        }
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_symlink_escape_is_denied() -> color_eyre::Result<()> {
        for absolute_link in [false, true] {
            let sandbox = SymlinkSandbox::new(absolute_link)?;
            assert!(sandbox
                .dir
                .create_dir_all(Path::new("escape/new/child"))
                .is_err());
            assert_absent(&sandbox.outside.join("new"));
            sandbox.assert_unchanged()?;

            let sandbox = SymlinkSandbox::new(absolute_link)?;
            assert!(sandbox
                .dir
                .create_new(Path::new("escape/new_file"))
                .is_err());
            assert_absent(&sandbox.outside.join("new_file"));
            sandbox.assert_unchanged()?;

            let sandbox = SymlinkSandbox::new(absolute_link)?;
            match sandbox.dir.open_read(Path::new("escape/sentinel")) {
                Err(_) => {}
                Ok(mut reader) => {
                    let mut bytes = Vec::new();
                    reader.read_to_end(&mut bytes)?;
                    panic!("escaping open_read returned outside bytes: {bytes:?}");
                }
            }
            sandbox.assert_unchanged()?;

            let sandbox = SymlinkSandbox::new(absolute_link)?;
            assert!(sandbox.dir.metadata(Path::new("escape/sentinel")).is_err());
            sandbox.assert_unchanged()?;

            let sandbox = SymlinkSandbox::new(absolute_link)?;
            assert!(sandbox
                .dir
                .remove_file(Path::new("escape/sentinel"))
                .is_err());
            sandbox.assert_unchanged()?;

            let sandbox = SymlinkSandbox::new(absolute_link)?;
            assert!(sandbox
                .dir
                .rename(Path::new("escape/sentinel"), Path::new("destination"),)
                .is_err());
            assert_absent(&sandbox.root.join("destination"));
            sandbox.assert_unchanged()?;

            let sandbox = SymlinkSandbox::new(absolute_link)?;
            std::fs::write(sandbox.root.join("source"), b"source bytes")?;
            assert!(sandbox
                .dir
                .rename(Path::new("source"), Path::new("escape/sentinel"),)
                .is_err());
            assert_eq!(std::fs::read(sandbox.root.join("source"))?, b"source bytes");
            sandbox.assert_unchanged()?;

            let sandbox = SymlinkSandbox::new(absolute_link)?;
            std::fs::write(sandbox.root.join("source"), b"source bytes")?;
            assert!(sandbox
                .dir
                .rename(Path::new("source"), Path::new("escape/new_file"),)
                .is_err());
            assert_eq!(std::fs::read(sandbox.root.join("source"))?, b"source bytes");
            assert_absent(&sandbox.outside.join("new_file"));
            sandbox.assert_unchanged()?;
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_symlink_within_root_supports_operations() -> color_eyre::Result<()> {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new("dir")?;
        let root = std::path::absolute(tmp.path())?;
        std::fs::create_dir(root.join("inside"))?;
        std::fs::write(root.join("inside/sentinel"), b"inside sentinel")?;
        symlink("inside", root.join("link"))?;
        let dir = Dir::new(&root)?;

        dir.create_dir_all(Path::new("link/new_dir/child"))?;
        assert!(std::fs::metadata(root.join("inside/new_dir/child"))?.is_dir());
        let child_names = std::fs::read_dir(root.join("inside/new_dir"))?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        assert_eq!(child_names, vec![std::ffi::OsString::from("child")]);

        let mut writer = dir.create_new(Path::new("link/new_file"))?;
        writer.write_all(b"created bytes")?;
        drop(writer);
        assert_eq!(
            std::fs::read(root.join("inside/new_file"))?,
            b"created bytes"
        );

        let mut bytes = Vec::new();
        dir.open_read(Path::new("link/sentinel"))?
            .read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"inside sentinel");
        assert!(dir.metadata(Path::new("link/sentinel"))? == Kind::File);
        assert!(dir.metadata(Path::new("link/new_dir/child"))? == Kind::Directory);

        dir.remove_file(Path::new("link/new_file"))?;
        assert_absent(&root.join("inside/new_file"));

        std::fs::write(root.join("inside/from_link"), b"source bytes")?;
        dir.rename(Path::new("link/from_link"), Path::new("moved_from_link"))?;
        assert_absent(&root.join("inside/from_link"));
        assert_eq!(
            std::fs::read(root.join("moved_from_link"))?,
            b"source bytes"
        );

        std::fs::write(root.join("source"), b"destination bytes")?;
        dir.rename(Path::new("source"), Path::new("link/at_link"))?;
        assert_absent(&root.join("source"));
        assert_eq!(
            std::fs::read(root.join("inside/at_link"))?,
            b"destination bytes"
        );
        assert_eq!(
            std::fs::read(root.join("inside/sentinel"))?,
            b"inside sentinel"
        );
        assert!(std::fs::symlink_metadata(root.join("link"))?
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_link(root.join("link"))?, Path::new("inside"));

        let mut root_names = std::fs::read_dir(&root)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        root_names.sort();
        assert_eq!(
            root_names,
            ["inside", "link", "moved_from_link"].map(std::ffi::OsString::from)
        );
        let mut inside_names = std::fs::read_dir(root.join("inside"))?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        inside_names.sort();
        assert_eq!(
            inside_names,
            ["at_link", "new_dir", "sentinel"].map(std::ffi::OsString::from)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn escaping_final_symlink_is_not_followed_or_replaced_through() -> color_eyre::Result<()> {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new("dir")?;
        let parent = std::path::absolute(tmp.path())?;
        let root = parent.join("root");
        let outside = parent.join("outside");
        std::fs::create_dir(&root)?;
        std::fs::create_dir(&outside)?;
        std::fs::write(root.join("inside"), b"inside sentinel")?;
        std::fs::write(outside.join("sentinel"), b"outside sentinel")?;
        let target = Path::new("../outside/sentinel");
        symlink(target, root.join("link"))?;
        let dir = Dir::new(&root)?;

        match dir.open_read(Path::new("link")) {
            Err(_) => {}
            Ok(mut reader) => {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                panic!("final symlink open_read returned outside bytes: {bytes:?}");
            }
        }
        assert!(dir.create_new(Path::new("link")).is_err());
        assert!(dir.metadata(Path::new("link"))? == Kind::Other);
        assert!(std::fs::symlink_metadata(root.join("link"))?
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_link(root.join("link"))?, target);
        assert_eq!(
            std::fs::read(outside.join("sentinel"))?,
            b"outside sentinel"
        );

        dir.rename(Path::new("link"), Path::new("moved_link"))?;
        assert_absent(&root.join("link"));
        assert!(std::fs::symlink_metadata(root.join("moved_link"))?
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_link(root.join("moved_link"))?, target);
        assert_eq!(
            std::fs::read(outside.join("sentinel"))?,
            b"outside sentinel"
        );

        dir.remove_file(Path::new("moved_link"))?;
        assert_absent(&root.join("moved_link"));
        assert_eq!(
            std::fs::read(outside.join("sentinel"))?,
            b"outside sentinel"
        );

        symlink(target, root.join("link"))?;
        std::fs::write(root.join("source"), b"replacement bytes")?;
        dir.rename(Path::new("source"), Path::new("link"))?;
        assert_absent(&root.join("source"));
        assert!(std::fs::symlink_metadata(root.join("link"))?
            .file_type()
            .is_file());
        assert_eq!(std::fs::read(root.join("link"))?, b"replacement bytes");
        assert_eq!(
            std::fs::read(outside.join("sentinel"))?,
            b"outside sentinel"
        );
        assert_eq!(std::fs::read(root.join("inside"))?, b"inside sentinel");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn opened_root_survives_pathname_replacement() -> color_eyre::Result<()> {
        let tmp = TempDir::new("dir")?;
        let parent = std::path::absolute(tmp.path())?;
        let root = parent.join("root");
        let moved = parent.join("moved_root");
        std::fs::create_dir(&root)?;
        std::fs::write(root.join("read_me"), b"original read")?;
        std::fs::write(root.join("type_probe"), b"original file")?;
        std::fs::write(root.join("remove_me"), b"original removal")?;
        std::fs::write(root.join("from_original"), b"original source")?;
        std::fs::write(root.join("from_destination"), b"original second source")?;
        std::fs::write(root.join("into_original"), b"original old destination")?;

        let dir = Dir::new(&root)?;
        std::fs::rename(&root, &moved)?;
        std::fs::create_dir(&root)?;
        std::fs::write(root.join("read_me"), b"replacement read")?;
        std::fs::create_dir(root.join("type_probe"))?;
        std::fs::write(root.join("type_probe/marker"), b"replacement directory")?;
        std::fs::write(root.join("remove_me"), b"replacement removal")?;
        std::fs::write(root.join("from_original"), b"replacement source")?;
        std::fs::write(root.join("to_original"), b"replacement first destination")?;
        std::fs::write(root.join("from_destination"), b"replacement second source")?;
        std::fs::write(
            root.join("into_original"),
            b"replacement second destination",
        )?;

        let mut bytes = Vec::new();
        dir.open_read(Path::new("read_me"))?
            .read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"original read");
        assert_eq!(std::fs::read(moved.join("read_me"))?, b"original read");
        assert_eq!(std::fs::read(root.join("read_me"))?, b"replacement read");

        assert!(dir.metadata(Path::new("type_probe"))? == Kind::File);
        assert!(std::fs::metadata(moved.join("type_probe"))?.is_file());
        assert_eq!(std::fs::read(moved.join("type_probe"))?, b"original file");
        assert!(std::fs::metadata(root.join("type_probe"))?.is_dir());
        assert_eq!(
            std::fs::read(root.join("type_probe/marker"))?,
            b"replacement directory"
        );

        dir.create_dir_all(Path::new("new_dir/nested"))?;
        assert!(std::fs::metadata(moved.join("new_dir/nested"))?.is_dir());
        assert_absent(&root.join("new_dir"));

        let mut writer = dir.create_new(Path::new("new_file"))?;
        writer.write_all(b"created under original")?;
        drop(writer);
        assert_eq!(
            std::fs::read(moved.join("new_file"))?,
            b"created under original"
        );
        assert_absent(&root.join("new_file"));

        dir.rename(Path::new("from_original"), Path::new("to_original"))?;
        assert_absent(&moved.join("from_original"));
        assert_eq!(
            std::fs::read(moved.join("to_original"))?,
            b"original source"
        );
        assert_eq!(
            std::fs::read(root.join("from_original"))?,
            b"replacement source"
        );
        assert_eq!(
            std::fs::read(root.join("to_original"))?,
            b"replacement first destination"
        );

        dir.rename(Path::new("from_destination"), Path::new("into_original"))?;
        assert_absent(&moved.join("from_destination"));
        assert_eq!(
            std::fs::read(moved.join("into_original"))?,
            b"original second source"
        );
        assert_eq!(
            std::fs::read(root.join("from_destination"))?,
            b"replacement second source"
        );
        assert_eq!(
            std::fs::read(root.join("into_original"))?,
            b"replacement second destination"
        );

        dir.remove_file(Path::new("remove_me"))?;
        assert_absent(&moved.join("remove_me"));
        assert_eq!(
            std::fs::read(root.join("remove_me"))?,
            b"replacement removal"
        );

        let mut moved_names = std::fs::read_dir(&moved)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        moved_names.sort();
        assert_eq!(
            moved_names,
            [
                "into_original",
                "new_dir",
                "new_file",
                "read_me",
                "to_original",
                "type_probe"
            ]
            .map(std::ffi::OsString::from),
        );
        let nested_names = std::fs::read_dir(moved.join("new_dir"))?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        assert_eq!(nested_names, [std::ffi::OsString::from("nested")]);
        let mut replacement_names = std::fs::read_dir(&root)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        replacement_names.sort();
        assert_eq!(
            replacement_names,
            [
                "from_destination",
                "from_original",
                "into_original",
                "read_me",
                "remove_me",
                "to_original",
                "type_probe"
            ]
            .map(std::ffi::OsString::from),
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn in_root_hard_link_shares_data_with_outside_name() -> color_eyre::Result<()> {
        let tmp = TempDir::new("dir")?;
        let parent = std::path::absolute(tmp.path())?;
        let root = parent.join("root");
        let outside = parent.join("outside");
        std::fs::create_dir(&root)?;
        std::fs::create_dir(&outside)?;
        std::fs::write(outside.join("sentinel"), b"outside original")?;
        std::fs::write(outside.join("sibling"), b"outside sibling")?;
        std::fs::write(root.join("inside_sibling"), b"inside sibling")?;
        std::fs::hard_link(outside.join("sentinel"), root.join("alias"))?;
        let dir = Dir::new(&root)?;

        let mut bytes = Vec::new();
        dir.open_read(Path::new("alias"))?.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"outside original");
        assert!(dir.metadata(Path::new("alias"))? == Kind::File);
        assert!(std::fs::metadata(root.join("alias"))?.is_file());
        assert_eq!(
            std::fs::read(outside.join("sentinel"))?,
            b"outside original"
        );

        std::fs::write(outside.join("sentinel"), b"outside updated")?;
        bytes.clear();
        dir.open_read(Path::new("alias"))?.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"outside updated");
        assert_eq!(std::fs::read(root.join("alias"))?, b"outside updated");

        dir.rename(Path::new("alias"), Path::new("moved_alias"))?;
        assert_absent(&root.join("alias"));
        assert_eq!(std::fs::read(root.join("moved_alias"))?, b"outside updated");
        assert_eq!(std::fs::read(outside.join("sentinel"))?, b"outside updated");

        dir.remove_file(Path::new("moved_alias"))?;
        assert_absent(&root.join("moved_alias"));
        assert_eq!(std::fs::read(outside.join("sentinel"))?, b"outside updated");
        assert_eq!(std::fs::read(outside.join("sibling"))?, b"outside sibling");
        assert_eq!(
            std::fs::read(root.join("inside_sibling"))?,
            b"inside sibling"
        );
        let inside_names = std::fs::read_dir(&root)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        assert_eq!(inside_names, [std::ffi::OsString::from("inside_sibling")]);
        let mut outside_names = std::fs::read_dir(&outside)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        outside_names.sort();
        assert_eq!(
            outside_names,
            ["sentinel", "sibling"].map(std::ffi::OsString::from),
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn kind_does_not_follow_final_symlink() -> color_eyre::Result<()> {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new("dir")?;
        let dir = Dir::new(&std::path::absolute(tmp.path())?)?;
        std::fs::write(tmp.path().join("target"), b"data")?;
        symlink("target", tmp.path().join("link"))?;

        assert!(dir.metadata(Path::new("link"))? == Kind::Other);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn create_new_refuses_dangling_symlink() -> color_eyre::Result<()> {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new("dir")?;
        let dir = Dir::new(&std::path::absolute(tmp.path())?)?;
        let link = tmp.path().join("link");
        let target = tmp.path().join("missing");
        symlink("missing", &link)?;

        assert!(dir.create_new(Path::new("link")).is_err());
        assert!(std::fs::symlink_metadata(&link)?.file_type().is_symlink());
        assert_eq!(std::fs::read_link(&link)?, Path::new("missing"));
        assert_eq!(
            std::fs::symlink_metadata(&target).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "redox")))]
    #[test]
    fn unsupported_commit_validates_without_mutating_entries() -> io::Result<()> {
        let tmp = TempDir::new("dir")?;
        let root = tmp.path();
        let dir = Dir::new(&std::path::absolute(root)?)?;
        std::fs::write(root.join("candidate"), b"staging")?;
        std::fs::write(root.join("published"), b"destination")?;

        let error = match dir.commit(Path::new("candidate"), Path::new("published")) {
            Err(error) => error,
            Ok(_) => panic!("unsupported commit unexpectedly succeeded"),
        };
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_invalid_input(dir.commit(Path::new("../outside"), Path::new("published")));
        assert_invalid_input(dir.commit(Path::new("candidate"), Path::new("../outside")));
        assert_eq!(std::fs::read(root.join("candidate"))?, b"staging");
        assert_eq!(std::fs::read(root.join("published"))?, b"destination");
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "redox"))]
    mod commit_tests {
        use super::*;
        use std::sync::Barrier;

        #[test]
        fn created_moves_staging_and_existing_preserves_both_entries() -> io::Result<()> {
            let tmp = TempDir::new("dir")?;
            let root = tmp.path();
            let dir = Dir::new(&std::path::absolute(root)?)?;
            std::fs::write(root.join("candidate"), b"first")?;

            assert!(matches!(
                dir.commit(Path::new("./candidate"), Path::new("./published"))?,
                Outcome::Created
            ));
            assert_eq!(std::fs::read(root.join("published"))?, b"first");
            assert_eq!(
                std::fs::symlink_metadata(root.join("candidate"))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::NotFound
            );

            assert!(matches!(
                dir.commit(Path::new("published"), Path::new("published"))?,
                Outcome::Existing
            ));
            assert_eq!(std::fs::read(root.join("published"))?, b"first");

            std::fs::write(root.join("other"), b"second")?;
            assert!(matches!(
                dir.commit(Path::new("other"), Path::new("published"))?,
                Outcome::Existing
            ));
            assert_eq!(std::fs::read(root.join("other"))?, b"second");
            assert_eq!(std::fs::read(root.join("published"))?, b"first");

            std::fs::create_dir(root.join("directory"))?;
            assert!(matches!(
                dir.commit(Path::new("other"), Path::new("directory"))?,
                Outcome::Existing
            ));
            assert!(root.join("directory").is_dir());
            assert_eq!(std::fs::read(root.join("other"))?, b"second");
            Ok(())
        }

        #[cfg(unix)]
        #[test]
        fn trailing_directory_components_are_respected() -> io::Result<()> {
            use std::os::unix::fs::symlink;

            let tmp = TempDir::new("dir")?;
            let root = tmp.path();
            let dir = Dir::new(&std::path::absolute(root)?)?;
            std::fs::write(root.join("candidate"), b"candidate")?;

            for source in ["candidate/", "candidate/."] {
                assert!(dir
                    .commit(Path::new(source), Path::new("published"))
                    .is_err());
                assert_eq!(std::fs::read(root.join("candidate"))?, b"candidate");
                assert_absent(&root.join("published"));
            }
            assert!(dir
                .commit(Path::new("candidate"), Path::new("published/."))
                .is_err());
            assert_eq!(std::fs::read(root.join("candidate"))?, b"candidate");
            assert_absent(&root.join("published"));

            std::fs::create_dir(root.join("directory"))?;
            symlink("directory", root.join("link"))?;
            assert!(dir
                .commit(Path::new("link/."), Path::new("published"))
                .is_err());
            assert_eq!(
                std::fs::read_link(root.join("link"))?,
                Path::new("directory")
            );
            assert!(root.join("directory").is_dir());
            assert_absent(&root.join("published"));

            assert!(matches!(
                dir.commit(Path::new("candidate"), Path::new("published/"))?,
                Outcome::Created
            ));
            assert_absent(&root.join("candidate"));
            assert_eq!(std::fs::read(root.join("published"))?, b"candidate");
            Ok(())
        }

        #[test]
        fn competing_commits_preserve_every_loser() -> io::Result<()> {
            let tmp = TempDir::new("dir")?;
            let root = tmp.path();
            let dir = Dir::new(&std::path::absolute(root)?)?;
            let stages: Vec<_> = (0..4).map(|i| format!("candidate-{i}")).collect();
            for (i, stage) in stages.iter().enumerate() {
                std::fs::write(root.join(stage), [i as u8])?;
            }
            let barrier = Barrier::new(stages.len() + 1);
            let results = std::thread::scope(|scope| {
                let handles: Vec<_> = stages
                    .iter()
                    .map(|stage| {
                        let dir = &dir;
                        let barrier = &barrier;
                        scope.spawn(move || {
                            barrier.wait();
                            dir.commit(Path::new(stage), Path::new("published"))
                        })
                    })
                    .collect();
                barrier.wait();
                handles
                    .into_iter()
                    .map(|handle| handle.join().unwrap())
                    .collect::<Vec<_>>()
            });

            let mut winners = 0;
            for (i, (stage, result)) in stages.iter().zip(results).enumerate() {
                match result? {
                    Outcome::Created => {
                        winners += 1;
                        assert_eq!(std::fs::read(root.join("published"))?, [i as u8]);
                        assert_eq!(
                            std::fs::symlink_metadata(root.join(stage))
                                .unwrap_err()
                                .kind(),
                            io::ErrorKind::NotFound
                        );
                    }
                    Outcome::Existing => {
                        assert_eq!(std::fs::read(root.join(stage))?, [i as u8]);
                    }
                }
            }
            assert_eq!(winners, 1);
            Ok(())
        }

        #[test]
        fn both_paths_are_validated_before_filesystem_access() -> io::Result<()> {
            let tmp = TempDir::new("dir")?;
            let root = tmp.path();
            let dir = Dir::new(&std::path::absolute(root)?)?;
            std::fs::write(root.join("candidate"), b"protected")?;

            for invalid in ["", ".", "../outside", "/absolute", "sub/../candidate"] {
                assert_invalid_input(dir.commit(Path::new(invalid), Path::new("missing/target")));
                assert_invalid_input(dir.commit(Path::new("missing/source"), Path::new(invalid)));
            }
            assert_eq!(std::fs::read(root.join("candidate"))?, b"protected");
            assert_eq!(std::fs::read_dir(root)?.count(), 1);
            Ok(())
        }

        #[cfg(unix)]
        #[test]
        fn escaping_ancestor_links_are_rejected_on_both_sides() -> io::Result<()> {
            for absolute_link in [false, true] {
                let sandbox = SymlinkSandbox::new(absolute_link)?;
                std::fs::write(sandbox.root.join("candidate"), b"protected")?;
                assert!(sandbox
                    .dir
                    .commit(Path::new("escape/sentinel"), Path::new("published"))
                    .is_err());
                assert_absent(&sandbox.root.join("published"));
                assert!(sandbox
                    .dir
                    .commit(Path::new("candidate"), Path::new("escape/sentinel"))
                    .is_err());
                assert!(sandbox
                    .dir
                    .commit(Path::new("candidate"), Path::new("escape/new"))
                    .is_err());
                assert_absent(&sandbox.outside.join("new"));
                assert_eq!(std::fs::read(sandbox.root.join("candidate"))?, b"protected");
                sandbox.assert_unchanged()?;
            }
            Ok(())
        }

        #[cfg(unix)]
        #[test]
        fn in_root_ancestor_links_work() -> io::Result<()> {
            use std::os::unix::fs::symlink;

            let tmp = TempDir::new("dir")?;
            let root = tmp.path();
            std::fs::create_dir(root.join("inside"))?;
            symlink("inside", root.join("link"))?;
            std::fs::write(root.join("inside/candidate"), b"from link")?;
            std::fs::write(root.join("other"), b"into link")?;
            let dir = Dir::new(&std::path::absolute(root)?)?;

            assert!(matches!(
                dir.commit(Path::new("link/candidate"), Path::new("published"))?,
                Outcome::Created
            ));
            assert!(matches!(
                dir.commit(Path::new("other"), Path::new("link/published"))?,
                Outcome::Created
            ));
            assert_eq!(std::fs::read(root.join("published"))?, b"from link");
            assert_eq!(std::fs::read(root.join("inside/published"))?, b"into link");
            assert_eq!(std::fs::read_link(root.join("link"))?, Path::new("inside"));
            Ok(())
        }

        #[cfg(unix)]
        #[test]
        fn final_symlink_is_not_overwritten() -> io::Result<()> {
            use std::os::unix::fs::symlink;

            let tmp = TempDir::new("dir")?;
            let parent = std::path::absolute(tmp.path())?;
            let root = parent.join("root");
            std::fs::create_dir(&root)?;
            std::fs::write(parent.join("outside"), b"outside")?;
            std::fs::write(root.join("candidate"), b"candidate")?;
            let target = Path::new("../outside");
            symlink(target, root.join("link"))?;
            let dir = Dir::new(&root)?;

            assert!(matches!(
                dir.commit(Path::new("candidate"), Path::new("link"))?,
                Outcome::Existing
            ));
            assert_eq!(std::fs::read(root.join("candidate"))?, b"candidate");
            assert_eq!(std::fs::read_link(root.join("link"))?, target);
            assert_eq!(std::fs::read(parent.join("outside"))?, b"outside");
            Ok(())
        }

        #[cfg(unix)]
        #[test]
        fn opened_root_remains_the_commit_root_after_pathname_replacement() -> io::Result<()> {
            let tmp = TempDir::new("dir")?;
            let parent = std::path::absolute(tmp.path())?;
            let root = parent.join("root");
            let moved = parent.join("moved");
            std::fs::create_dir(&root)?;
            std::fs::create_dir(root.join("staging"))?;
            std::fs::write(root.join("staging/candidate"), b"original")?;
            let dir = Dir::new(&root)?;

            std::fs::rename(&root, &moved)?;
            std::fs::create_dir(&root)?;
            std::fs::create_dir(root.join("staging"))?;
            std::fs::write(root.join("staging/candidate"), b"replacement candidate")?;
            std::fs::write(root.join("published"), b"replacement destination")?;

            assert!(matches!(
                dir.commit(Path::new("staging/candidate"), Path::new("published"))?,
                Outcome::Created
            ));
            assert_eq!(std::fs::read(moved.join("published"))?, b"original");
            assert_absent(&moved.join("staging/candidate"));
            assert_eq!(
                std::fs::read(root.join("staging/candidate"))?,
                b"replacement candidate"
            );
            assert_eq!(
                std::fs::read(root.join("published"))?,
                b"replacement destination"
            );
            Ok(())
        }
    }
}
