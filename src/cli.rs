use std::{env, result};

/// Sub-commands for the CLI
#[derive(Debug)]
pub enum Cli {
    /// Initialise a directory for ssg to build on
    Init(String),
    /// Put some arbitrary bytes into the object storage
    Put(String),
    /// Get the bytes stored in the object storage associated with the given
    /// hash
    Get(String),
}

const COMMANDS: &[(&str, fn(String) -> Cli)] =
    &[("init", Cli::Init), ("put", Cli::Put), ("get", Cli::Get)];

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("No sub-command was provided!")]
    NoCommandProvided,
    #[error("Command not found: {command}")]
    CommandNotFound { command: String },
    #[error("Arguments to the subcommand are not provided")]
    ArgsNotProvided,
}

type Result<T> = result::Result<T, Error>;

impl Cli {
    /// Parse [`std::env::args`] into [`Cli`]
    pub(crate) fn parse() -> Result<Self> {
        let cmd = env::args()
            .nth(1)
            .map_or(Err(Error::NoCommandProvided), |name| {
                // Find known subcommands corresponding to this. This is
                // efficient since our table is small.
                let res = COMMANDS
                    .iter()
                    .find(|(cmd, _)| *cmd == name)
                    .map(|(_, cons)| *cons);

                res.ok_or(
                    // Can't find our command in the table, so it probably does not
                    // exist.
                    Error::CommandNotFound { command: name },
                )
            })?;
        let param = env::args().nth(2).ok_or(Error::ArgsNotProvided)?;
        Ok(cmd(param))
    }
}
