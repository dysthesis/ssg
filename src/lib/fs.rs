use cap_std::ambient_authority;
use cap_std::fs::{Dir as CapDir, OpenOptions};

use std::{
    fs::File,
    io::{self, Read, Result, Write},
    path::{Component, Path, PathBuf},
};

/// Filesystem interface for swappable backends, e.g. fault injection.
pub(crate) trait Fs: Send + Sync {
    type Reader: Read;
    type Writer: Write;

    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn create_new(&self, path: &Path) -> Result<Self::Writer>;
    fn open_read(&self, path: &Path) -> Result<Self::Reader>;
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<Kind>;
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
    root: PathBuf,
    cap_root: CapDir,
}

impl Dir {
    /// Open an existing absolute directory, retaining its supplied spelling.
    /// The spelling is not used for subsequent filesystem I/O.
    pub(crate) fn new(root: PathBuf) -> io::Result<Self> {
        if !root.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "root must be an absolute directory",
            ));
        }

        let cap_root = CapDir::open_ambient_dir(&root, ambient_authority())?;
        Ok(Self { root, cap_root })
    }

    #[inline]
    fn validate(path: &Path) -> io::Result<()> {
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

    /// Map a validated relative path to the supplied root spelling for tests.
    /// This path must never be used for filesystem I/O.
    #[cfg(test)]
    fn resolve(&self, path: &Path) -> io::Result<PathBuf> {
        Self::validate(path)?;
        Ok(self.root.join(path))
    }

    #[cfg(test)]
    fn root(&self) -> &Path {
        &self.root
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
                panic!("operation unexpectedly accepted an escaping path");
            }
        }
    }

    #[test]
    fn empty_path_is_rejected() -> color_eyre::Result<()> {
        let tmp = TempDir::new("dir")?;
        let root = std::path::absolute(tmp.path())?;
        let dir = Dir::new(root)?;

        let error = dir
            .resolve(Path::new(""))
            .expect_err("empty path must be rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        Ok(())
    }

    #[test]
    fn current_directory_path_is_rejected() -> color_eyre::Result<()> {
        let tmp = TempDir::new("dir")?;
        let root = std::path::absolute(tmp.path())?;
        let dir = Dir::new(root)?;

        let error = dir
            .resolve(Path::new("."))
            .expect_err("current-directory path must be rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        Ok(())
    }

    #[test]
    fn current_directory_components_work_in_operations() -> color_eyre::Result<()> {
        let tmp = TempDir::new("dir")?;
        let dir = Dir::new(std::path::absolute(tmp.path())?)?;

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

            let dir = Dir::new(root.clone())?;

            prop_assert_eq!(dir.root(), root.as_path());
        }

        #[test]
        fn nonexistent_relative_root_is_rejected(
            root in relative_path(),
        ) {
            prop_assume!(!root.exists());

            let error = Dir::new(root).unwrap_err();

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

            let error = Dir::new(relative).unwrap_err();

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
            prop_assert!(Dir::new(root).is_err());
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

            prop_assert!(Dir::new(root.clone()).is_err());

            prop_assert_eq!(
                std::fs::read(root).unwrap(),
                bytes
            );
        }

        /// Check that [`Dir`] does not rewrite relative paths
        #[test]
        fn valid_relative_path_is_preserved(
            path in relative_path(),
        ) {
            let tmp = tempdir::TempDir::new("dir").unwrap();
            let root = std::path::absolute(tmp.path()).unwrap();

            let dir = Dir::new(root.clone()).unwrap();

            let resolved = dir.resolve(&path).unwrap();

            prop_assert_eq!(
                resolved.strip_prefix(&root).unwrap(),
                path.as_path(),
            );
        }

        // Dir traversal prevention tests
        #[test]
        fn parent_components_are_rejected(
            path in parent_path(),
        ) {
            let tmp = tempdir::TempDir::new("dir").unwrap();
            let root = std::path::absolute(tmp.path()).unwrap();

            let dir = Dir::new(root).unwrap();

            let error = dir.resolve(&path).unwrap_err();

            prop_assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
            );
        }

        #[test]
        fn absolute_paths_are_rejected(
            relative in relative_path(),
        ) {
            let tmp = tempdir::TempDir::new("dir").unwrap();
            let root = std::path::absolute(tmp.path()).unwrap();

            let dir = Dir::new(root.clone()).unwrap();

            let absolute = root.join(relative);

            let error = dir.resolve(&absolute).unwrap_err();

            prop_assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
            );
        }

        #[test]
        fn open_read_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let tmp = TempDir::new("dir").unwrap();
            let parent = std::path::absolute(tmp.path()).unwrap();
            let root = parent.join("root");
            std::fs::create_dir(&root).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, &bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = parent.join(&sibling);
            std::fs::write(&outside, &bytes).unwrap();
            let dir = Dir::new(root.clone()).unwrap();

            assert_invalid_input(dir.open_read(&Path::new("..").join(&sibling)));

            prop_assert!(root.is_dir());
            prop_assert_eq!(std::fs::read(&inside).unwrap(), bytes.clone());
            prop_assert_eq!(std::fs::read(&outside).unwrap(), bytes);
        }

        #[test]
        fn metadata_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let tmp = TempDir::new("dir").unwrap();
            let parent = std::path::absolute(tmp.path()).unwrap();
            let root = parent.join("root");
            std::fs::create_dir(&root).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, &bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = parent.join(&sibling);
            std::fs::write(&outside, &bytes).unwrap();
            let dir = Dir::new(root.clone()).unwrap();

            assert_invalid_input(dir.metadata(&Path::new("..").join(&sibling)));

            prop_assert!(root.is_dir());
            prop_assert_eq!(std::fs::read(&inside).unwrap(), bytes.clone());
            prop_assert_eq!(std::fs::read(&outside).unwrap(), bytes);
        }

        #[test]
        fn remove_file_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let tmp = TempDir::new("dir").unwrap();
            let parent = std::path::absolute(tmp.path()).unwrap();
            let root = parent.join("root");
            std::fs::create_dir(&root).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, &bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = parent.join(&sibling);
            std::fs::write(&outside, &bytes).unwrap();
            let dir = Dir::new(root.clone()).unwrap();

            assert_invalid_input(dir.remove_file(&Path::new("..").join(&sibling)));

            prop_assert!(root.is_dir());
            prop_assert_eq!(std::fs::read(&inside).unwrap(), bytes.clone());
            prop_assert_eq!(std::fs::read(&outside).unwrap(), bytes);
        }

        #[test]
        fn create_new_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let tmp = TempDir::new("dir").unwrap();
            let parent = std::path::absolute(tmp.path()).unwrap();
            let root = parent.join("root");
            std::fs::create_dir(&root).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, &bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = parent.join(&sibling);
            prop_assert!(!outside.exists());
            let dir = Dir::new(root.clone()).unwrap();

            assert_invalid_input(dir.create_new(&Path::new("..").join(&sibling)));

            prop_assert!(root.is_dir());
            prop_assert_eq!(std::fs::read(&inside).unwrap(), bytes);
            prop_assert!(!outside.exists());
        }

        #[test]
        fn create_dir_all_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let tmp = TempDir::new("dir").unwrap();
            let parent = std::path::absolute(tmp.path()).unwrap();
            let root = parent.join("root");
            std::fs::create_dir(&root).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, &bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = parent.join(&sibling);
            prop_assert!(!outside.exists());
            let dir = Dir::new(root.clone()).unwrap();

            assert_invalid_input(dir.create_dir_all(&Path::new("..").join(&sibling)));

            prop_assert!(root.is_dir());
            prop_assert_eq!(std::fs::read(&inside).unwrap(), bytes);
            prop_assert!(!outside.exists());
        }

        #[test]
        fn rename_source_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let tmp = TempDir::new("dir").unwrap();
            let parent = std::path::absolute(tmp.path()).unwrap();
            let root = parent.join("root");
            std::fs::create_dir(&root).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, &bytes).unwrap();
            let destination = root.join("destination");
            prop_assert!(!destination.exists());
            let sibling = format!("outside-{name}");
            let outside = parent.join(&sibling);
            std::fs::write(&outside, &bytes).unwrap();
            let dir = Dir::new(root.clone()).unwrap();

            assert_invalid_input(dir.rename(
                &Path::new("..").join(&sibling),
                Path::new("destination"),
            ));

            prop_assert!(root.is_dir());
            prop_assert_eq!(std::fs::read(&inside).unwrap(), bytes.clone());
            prop_assert_eq!(std::fs::read(&outside).unwrap(), bytes);
            prop_assert!(!destination.exists());
        }

        #[test]
        fn rename_destination_cannot_escape_root(
            name in component(),
            bytes in prop::collection::vec(any::<u8>(), 0..=4096),
        ) {
            let tmp = TempDir::new("dir").unwrap();
            let parent = std::path::absolute(tmp.path()).unwrap();
            let root = parent.join("root");
            std::fs::create_dir(&root).unwrap();
            let inside = root.join("inside");
            std::fs::write(&inside, &bytes).unwrap();
            let sibling = format!("outside-{name}");
            let outside = parent.join(&sibling);
            prop_assert!(!outside.exists());
            let dir = Dir::new(root.clone()).unwrap();

            assert_invalid_input(dir.rename(
                Path::new("inside"),
                &Path::new("..").join(&sibling),
            ));

            prop_assert!(root.is_dir());
            prop_assert_eq!(std::fs::read(&inside).unwrap(), bytes);
            prop_assert!(!outside.exists());
        }
    }

    proptest! {
        #[test]
        fn create_dir_all_creates_every_component(path in relative_path()) {
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;

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
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;

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
            let dir = Arc::new(Dir::new(std::path::absolute(tmp.path())?)?);
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
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;
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
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;
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
            let dir = Arc::new(Dir::new(std::path::absolute(tmp.path())?)?);
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
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;

            prop_assert_eq!(dir.open_read(&path).unwrap_err().kind(), io::ErrorKind::NotFound);
        }

        #[test]
        fn rename_moves_file_without_changing_bytes(
            from in component(), to in component(), bytes in any::<Vec<u8>>(),
        ) {
            prop_assume!(from != to);
            let tmp = TempDir::new("dir")?;
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;
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
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;
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
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;
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
            let dir = Dir::new(std::path::absolute(tmp.path())?)?;
            drop(dir.create_new(Path::new(&file))?);
            dir.create_dir_all(Path::new(&directory))?;

            prop_assert!(dir.metadata(Path::new(&file))? == Kind::File);
            prop_assert!(dir.metadata(Path::new(&directory))? == Kind::Directory);
        }
    }

    #[cfg(unix)]
    #[test]
    fn kind_does_not_follow_final_symlink() -> color_eyre::Result<()> {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new("dir")?;
        let dir = Dir::new(std::path::absolute(tmp.path())?)?;
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
        let dir = Dir::new(std::path::absolute(tmp.path())?)?;
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
}
