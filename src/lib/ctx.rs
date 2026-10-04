use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{
    db::{self, Db}, fs::Dir, policy::crit_len::CritLen, query::{self, Query, Registry}, runtime::Runtime, scheduler::Scheduler, store::{self, Store},
};

pub const NUM_THREADS: usize = 8;

pub struct Ctx {
    store: Store,
    db: Db,
    queries: Registry,
    runtime: Runtime<NUM_THREADS>,
    scheduler: Scheduler<CritLen>,
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
        let runtime = Runtime::<NUM_THREADS>::init();
        let scheduler = Scheduler::new();
        Ok(Self {
            store,
            db,
            queries,
            scheduler,
            runtime,
        })
    }
    pub fn run<Q: Query>(&self, query: Q) {
        // TODO: actual execution, caching logic
        // TODO: make it sync lmao
        query.query(&self.store);
    }
    /// Queries should call this instead
    pub async fn query<Q: Query>(&self, from: query::Id, what: query:Id) -> store::Id {
        todo!()
    }
}
