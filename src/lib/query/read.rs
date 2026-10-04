use std::{borrow::Cow, fs::read, os::unix::ffi::OsStrExt, path::PathBuf};

use crate::{query::Query, store};

#[derive(Hash)]
pub struct Read(PathBuf);

impl Query for Read {
    const NAME: &'static str = "read";
    const VERSION: u64 = 1;

    fn params(&self) -> Vec<Cow<'_, [u8]>> {
        vec![Cow::Borrowed(self.0.as_os_str().as_bytes())]
    }

    async fn query(&self, store: &crate::store::Store) -> store::Id {
        let data = read(&self.0).unwrap_or_else(|err| panic!("failed to read {:?}: {err}", self.0));
        store.put(&data).expect("Failed to put data to store")
    }
}

impl Read {
    pub fn new(file: PathBuf) -> Self {
        Self(file)
    }
}
