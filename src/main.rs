use color_eyre::{Section, eyre::Result};
use itertools::{Either, Itertools};
use libssg::document::{Buildable, Document, Parseable, Writeable};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{env::current_dir, fs::read_to_string};
use tracing::{error, info};
use walkdir::{DirEntry, WalkDir};

#[cfg_attr(all(not(debug_assertions), not(test)), no_panic::no_panic)]
fn main() -> Result<()> {
    // Install error logging
    color_eyre::install()?;

    let input_dir =
        current_dir().with_note(|| "While getting current working directory for the input.")?;

    let output_dir = input_dir.join("out");

    let stylesheet = {
        let path = input_dir.join("style.css");
        if path.is_file() {
            match read_to_string(&path) {
                Ok(content) => Some(content),
                Err(error) => {
                    error!("Failed to read stylesheet {path:?}: {error}");
                    None
                }
            }
        } else {
            None
        }
    };

    println!(
        r#"
    Input directory:    {input_dir:?}
    Ouptut directory:   {output_dir:?}
    "#
    );

    info!("Enumerating directory entries...");
    let (dir_entries, errors): (Vec<DirEntry>, Vec<walkdir::Error>) = WalkDir::new(input_dir)
        .into_iter()
        .partition_map(|r| match r {
            Ok(v) => Either::Left(v),
            Err(e) => Either::Right(e),
        });

    // TODO: Better formatting for error vectors
    // Print out errors instead fo failing because we can still render the other
    // pages without them.
    if !errors.is_empty() {
        error!("Failed to open some directory entries: {errors:?}");
    }

    // Get all Markdown documents in the directory
    let (source_documents, errors): (Vec<(DirEntry, String)>, Vec<std::io::Error>) = dir_entries
        .into_iter()
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .partition_map(|e| match read_to_string(e.path()) {
            Ok(content) => Either::Left((e, content)),
            Err(e) => Either::Right(e),
        });

    // Print out errors instead fo failing because we can still render the other
    // pages without them.
    if !errors.is_empty() {
        error!("Failed to open some directory entries: {errors:?}");
    }

    let write_errors: Vec<color_eyre::Report> = source_documents
        .par_iter()
        .map(|(doc, content)| (doc.path().to_path_buf(), content))
        .map(|(doc, content)| Document::new(doc, content, stylesheet.clone()))
        .map(|doc| doc.parse())
        .filter_map(|parsed| parsed.build().ok())
        .filter_map(|html| html.write().err().map(color_eyre::Report::from))
        .collect();

    if !write_errors.is_empty() {
        error!("Failed to write some documents: {write_errors:?}");
    }

    Ok(())
}
