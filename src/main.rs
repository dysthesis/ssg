use color_eyre::eyre;

use crate::cli::Cli;

mod cli;
fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse()?;
    println!("{cli:?}");
    Ok(())
}
