use color_eyre::{Section, eyre::Result};
use std::{env::current_dir, fs::read_to_string};
use walkdir::{DirEntry, WalkDir};

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

    let source_documents: Vec<(DirEntry, String)> = WalkDir::new(input_dir)
        .into_iter()
        // TODO: See if we want to log erroneous entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .filter_map(|e| read_to_string(e.path()).ok().map(|content| (e, content)))
        .collect();

    Ok(())
}
