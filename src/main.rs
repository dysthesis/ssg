use std::{
    env::current_dir,
    fs::{self, read_to_string},
};

use color_eyre::{Section, eyre::eyre};
use itertools::{Either, Itertools};
use pulldown_cmark::{Options, Parser};
use ssg::{
    front_matter::FrontMatter,
    transformer::{WithTransformer, code_block::CodeHighlightTransformer},
};
use walkdir::{DirEntry, WalkDir};

const INPUT_DIR: &str = "contents";
const OUPTPUT_DIR: &str = "out";

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let current_dir = current_dir().with_note(|| "While getting the current working directory")?;
    let input_dir = current_dir.join(INPUT_DIR);
    let output_dir = current_dir.join(OUPTPUT_DIR);

    let (dir_entries, errors): (Vec<DirEntry>, Vec<walkdir::Error>) = WalkDir::new(&input_dir)
        .into_iter()
        .partition_map(|r| match r {
            Ok(v) => Either::Left(v),
            Err(e) => Either::Right(e),
        });

    if !errors.is_empty() {
        return Err(eyre!("Failed to open some directory entries: {errors:?}"));
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

    if !errors.is_empty() {
        return Err(eyre!("Failed to open some files: {errors:?}"));
    }

    // Parse all the documents. We first construct the options with which to
    // parse, i.e. the features to enable.
    let mut options = Options::empty();
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_SUPERSCRIPT);
    options.insert(Options::ENABLE_SUBSCRIPT);

    let footer = read_to_string(current_dir.join("footer").with_extension("html"))
        .with_note(|| "While reading HTML footer")?;

    source_documents
        .into_iter()
        .map(|(path, content)| {
            let header = dbg!(FrontMatter::try_from(content.as_str()))
                .map(|res| res.to_html())
                .unwrap_or_default();
            let parser = Parser::new_ext(content.as_str(), options)
                .with_transformer::<CodeHighlightTransformer<'_, _>>();
            let mut html_output = String::new();
            pulldown_cmark::html::push_html(&mut html_output, parser);
            println!("Rendered {html_output}");
            (path, html_output, header)
        })
        .filter_map(|(path, rendered, header)| {
            let rel = path.path().strip_prefix(&input_dir).ok()?;
            Some((
                output_dir.join(rel).with_extension("html"),
                rendered,
                header,
            ))
        })
        .for_each(|(out_path, rendered, header)| {
            let html = format!(
                r#"
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<head>
{header}
</head>
<body>
{rendered}
</body>
{footer}"#
            );
            // TODO: Error handling
            _ = fs::write(out_path, html);
            _ = fs::copy(
                input_dir.join("style").with_extension("css"),
                output_dir.join("style").with_extension("css"),
            );
        });

    Ok(())
}
