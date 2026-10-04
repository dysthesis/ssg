//! Observations from previous builds are stored in an SQLite database, which is
//! managed by this module. For each query, we record,
//!
//! - what its output hash is in the last build, and
//! - what dependencies it requested, and what their hashes were.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::{query, store};

/// A handle to an SQLite database that provides domain logic to put and get
/// the previous build trace for a given query.
pub struct Db {
    conn: Connection,
}

/// Persistent row ID in the database corresponding to some trace.
pub struct Id(i64);

/// Observed dependency of some query during the last build
pub struct Dependency {
    dep: Id,
    expected: store::Id,
}

/// The trace of the most recent successful build for this query
pub struct Trace {
    /// The identifier of the query in question
    query: query::Key,
    /// The output that this execution of the query produced.
    output: store::Id,
    /// A list of dependencies that this query has been observed to require
    /// during the last execution.
    dependencies: Vec<Dependency>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to open the database in {path}.")]
    DbOpenError {
        path: PathBuf,
        #[source]
        error: rusqlite::Error,
    },
    #[error("Failed to migrate database at {path}.")]
    Migration {
        path: PathBuf,
        #[source]
        error: rusqlite::Error,
    },
}

type Result<T> = std::result::Result<T, Error>;

const V1_MIGRATION: &str = include_str!("./statements/migrations/v1.sql");

impl Db {
    pub fn init(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path).map_err(|error| Error::DbOpenError {
            path: path.to_path_buf(),
            error,
        })?;

        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| Error::Migration {
                path: path.to_owned(),
                error,
            })?;

        match version {
            0 => {
                let tx = conn.transaction().map_err(|error| Error::Migration {
                    path: path.to_owned(),
                    error,
                })?;

                tx.execute_batch(V1_MIGRATION)
                    .map_err(|error| Error::Migration {
                        path: path.to_owned(),
                        error,
                    })?;

                tx.pragma_update(None, "user_version", 1)
                    .map_err(|error| Error::Migration {
                        path: path.to_owned(),
                        error,
                    })?;

                tx.commit().map_err(|error| Error::Migration {
                    path: path.to_owned(),
                    error,
                })?;
            }
            1 => {
                // Already current.
            }
            version => {
                todo!("unsupported database version {version}");
            }
        }
        Ok(Self { conn })
    }
}
