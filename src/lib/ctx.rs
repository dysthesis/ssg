use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{
    db::{self, Db},
    fs::Dir,
    query::Registry,
    store::{self, Store},
};

pub struct Ctx {
    store: Store,
    db: Db,
    queries: Registry,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to open directory at {path}.")]
    DirOpenError {
        path: PathBuf,
        #[source]
        error: io::Error,
    },
    #[error("Failed to open the object storage at {path}")]
    StoreInitError {
        path: PathBuf,
        #[source]
        error: store::Error,
    },
    #[error("Failed to initalise database at {path}")]
    DbInitError {
        path: PathBuf,
        #[source]
        error: db::Error,
    },
}

type Result<T> = std::result::Result<T, Error>;

impl Ctx {
    pub fn new(store_path: &Path, db_path: &Path) -> Result<Self> {
        let dir = Dir::new(store_path).map_err(|error| Error::DirOpenError {
            path: store_path.to_path_buf(),
            error,
        })?;
        let store = Store::init(dir).map_err(|error| Error::StoreInitError {
            path: store_path.to_path_buf(),
            error,
        })?;
        let db = Db::init(db_path).map_err(|error| Error::DbInitError {
            path: db_path.to_path_buf(),
            error,
        })?;
        let queries = Registry::new();
        Ok(Self { store, db, queries })
    }
}
