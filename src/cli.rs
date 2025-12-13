use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct Cli {
    #[arg(short, long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    #[command(subcommand)]
    pub commmand: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    Build {
        /// Where the build resuts should be outputted
        output_dir: Option<PathBuf>,
    },
}
