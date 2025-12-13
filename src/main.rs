use color_eyre::{Section, eyre::Result};
use std::env::current_dir;

fn main() -> Result<()> {
    // Install error logging
    color_eyre::install()?;

    let input_dir =
        current_dir().with_note(|| "While getting current working directory for the input.")?;

    let output_dir = input_dir.join("result");

    println!(
        r#"
    Input directory:    {input_dir:?}
    Ouptut directory:   {output_dir:?}
    "#
    );

    Ok(())
}
