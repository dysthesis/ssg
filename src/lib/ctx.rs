use std::{
    io, path::{Path, PathBuf}, task::Poll,
};

use crate::{
    db::{self, Db}, fs::Dir, graph::Graph, policy::crit_len::CritLen, query::{self, Erased, Query, Registry}, runtime::{Event, Runtime}, scheduler::{Scheduler, Task}, store::{self, Store},
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
    /// Register a query to the [`Self::queries`] registry if it does not exist.
    /// In either case, returns the corresponding [`query::Id`].
    pub fn register_query<Q: Query>(&self, query: Q) -> query::Id {
        todo!()
    }

    /// Get the query associated with the given id.
    #[inline]
    pub fn get_query(&self, id: query::Id) -> Option<&dyn Erased> {
        self.queries.get(id)
    }
    
pub fn run(&mut self, root: query::Id) -> store::Id {
    // The root has no dependency preventing it from running.
    let task = Task::new(root, &self);
    self.scheduler.submit(task);

    loop {
        // Give every currently runnable task to the worker pool.
        while let Some(task) = self.scheduler.next() {
            self.runtime.submit(task);
        }

        // Nothing else is immediately runnable.
        // Sleep until a worker or waker gives us new information.
        match self.runtime.recv() {
            Event::Polled(task, Poll::Ready(output)) => {
                let id = task.query;

                self.scheduler.complete(task, output);

                if id == root {
                    return output;
                }
            }

            Event::Polled(task, Poll::Pending) => {
                self.scheduler.park(task);
            }

            Event::Wake(id) => {
                self.scheduler.wake(&id);
            }
        }
    }
}
    /// Queries should call this instead
    pub async fn query(&self, from: query::Id, what: query::Id) -> store::Id {
        // TODO: Register this as a task in the scheduler and wait to be woken 
        // up upon completion
        self.result_for(what)
    }

    /// Checks the most recent result for some [`query::Query`] that is recorded
    /// in [`Self::db`] and returns the corresponding [`store::Id`].
    pub fn result_for(&self, query: query::Id) -> store::Id {
        todo!()
    }

    /// Report a query as completed
    pub fn complete(&self, query: query::Id) {
        todo!()
    }

    /// Unblock a query when its dependency is ready
    pub(crate) fn unblock(&self, query: query::Id) {
        todo!()
    }
}
