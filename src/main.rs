use std::{
    collections::BTreeMap,
    env::current_dir,
    fs::{self, read_to_string},
    path::PathBuf,
};

use color_eyre::{Section, eyre::eyre};
use itertools::{Either, Itertools};
use pulldown_cmark::{Options, Parser};
use ssg::{
    article::{Article, render_listing_page},
    css::build_css,
    header::Header,
    transformer::{
        WithTransformer,
        code_block::CodeHighlightTransformer,
        footnote::FootnoteTransformer,
        heading::HeadingDemoterTransformer,
        math::MathTransformer,
        toc::{TocTransformer, escape_attr},
    },
};
use walkdir::{DirEntry, WalkDir};

const INPUT_DIR: &str = "contents";
const OUPTPUT_DIR: &str = "public";
const POSTS_DIR: &str = "posts";
const TAGS_DIR: &str = "tags";

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let current_dir = current_dir().with_note(|| "While getting the current working directory")?;
    let input_dir = current_dir.join(INPUT_DIR);
    let output_dir = current_dir.join(OUPTPUT_DIR);

    fs::create_dir_all(&output_dir)?;

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

    let head =
        read_to_string(current_dir.join("header").with_extension("html")).unwrap_or_default();
    let footer =
        read_to_string(current_dir.join("footer").with_extension("html")).unwrap_or_default();

    let mut articles: Vec<Article> = Vec::new();

    source_documents
        .into_iter()
        .filter_map(|(entry, content)| {
            let rel_src = entry.path().strip_prefix(&input_dir).ok()?.to_owned();

            let rel_out = PathBuf::from(POSTS_DIR)
                .join(&rel_src)
                .with_extension("html");
            let out_path = output_dir.join(&rel_out);

            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).ok()?;
            }

            let href = path_to_href(&rel_out);
            let prefix = prefix_to_root(&rel_out);
            let css_href = format!("{prefix}style.css");

            let header = Header::try_from(content.as_str()).unwrap_or_default();
            let head_fragment = header.to_html(&css_href);
            let body_header = header.generate_body_head(&prefix);

            let parser = Parser::new_ext(content.as_str(), options)
                .with_transformer::<CodeHighlightTransformer<'_, _>>()
                .with_transformer::<MathTransformer<'_, _>>()
                .with_transformer::<FootnoteTransformer<'_>>()
                .with_transformer::<HeadingDemoterTransformer<'_, _>>()
                .with_transformer::<TocTransformer<'_>>();

            let mut rendered = String::new();
            pulldown_cmark::html::push_html(&mut rendered, parser);

            rendered.push_str(&format!(
                r#"
<p class="meta"><a href="{0}index.html">Index</a></p>
"#,
                escape_attr(&prefix)
            ));

            let title = header
                .title()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| rel_out.to_string_lossy().to_string());

            if let Some(ctime) = header.ctime() {
                articles.push(Article {
                    title,
                    ctime: ctime.to_owned(),
                    href,
                    tags: header.tags().to_vec(),
                });
            }

            Some((out_path, rendered, head_fragment, body_header))
        })
        .for_each(|(out_path, rendered, header_html, body_header)| {
            let html = format!(
                r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
{head}
{header_html}
</head>
<body>
<article>
<section>
{body_header}
{rendered}
</section>
</article>
</body>
</html>
"#
            );
            _ = fs::write(out_path, html);
        });

    articles.sort_by(|a, b| b.ctime.cmp(&a.ctime).then_with(|| a.title.cmp(&b.title)));
    let index_rel = std::path::Path::new("index.html");
    let index_prefix = prefix_to_root(index_rel);

    let index_html = render_listing_page("Index", "Index", &articles, &head, &index_prefix);

    fs::write(output_dir.join("index.html"), index_html)?;

    let mut by_tag: BTreeMap<String, Vec<Article>> = BTreeMap::new();
    for a in &articles {
        for t in &a.tags {
            by_tag.entry(t.clone()).or_default().push(a.clone());
        }
    }

    let tags_dir = output_dir.join(TAGS_DIR);
    fs::create_dir_all(&tags_dir)?;
    for (tag, mut tagged) in by_tag {
        let tag_rel = std::path::PathBuf::from(TAGS_DIR).join(format!("{tag}.html"));
        let tag_prefix = prefix_to_root(&tag_rel);

        let html = render_listing_page(
            &format!("Tag: {tag}"),
            &format!("Tag: {tag}"),
            &tagged,
            &head,
            &tag_prefix,
        );

        fs::write(output_dir.join(tag_rel), html)?;
    }

    // Minify and copy over style.css
    let stylesheet_in_path = current_dir.join("style").with_extension("css");
    let stylesheet_out_path = output_dir.join("style").with_extension("css");
    let stylesheet = build_css(stylesheet_in_path.as_path())?;

    fs::write(stylesheet_out_path, stylesheet)?;

    Ok(())
}
fn prefix_to_root(rel_out: &std::path::Path) -> String {
    let depth = rel_out
        .parent()
        .map(|p| p.components().count())
        .unwrap_or(0);
    "../".repeat(depth)
}

fn path_to_href(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}
