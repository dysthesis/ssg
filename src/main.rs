use clap::Parser;
use color_eyre::{
    Section,
    eyre::{Context, Result},
};

use crate::cli::Cli;

mod cli;
fn main() -> Result<()> {
    // Install error logging
    color_eyre::install()?;

    // Process CLI arguments
    let cli = Cli::parse();
    match cli.commmand {
        cli::Command::Build { output_dir } => {
            let input_dir = cli
                .dir
                .map_or_else(|| std::env::current_dir()
                    .wrap_err("Failed to find the current working directory")
                    .with_note(|| "No input path was provided; attempted to fallback to the current working directory"), Ok)?;

            let output_dir = output_dir.unwrap_or_else(|| input_dir.join("result"));

            println!(
                r#"
            Output dirctory:    {output_dir:?}
            Input directory:    {input_dir:?}
            "#
            )
        }
    }

    Ok(())
}
