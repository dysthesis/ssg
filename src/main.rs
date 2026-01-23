use std::{
    collections::BTreeMap,
    env::{self, current_dir},
    fs::{self, read_to_string},
    io,
    path::{Path, PathBuf},
};

use axum::Router;
use color_eyre::{Section, eyre::eyre};
use itertools::{Either, Itertools};
use notify::{EventKind, RecursiveMode, Watcher};
use pulldown_cmark::{Event, Options, Parser};
use ssg::{
    article::{Article, render_listing_page},
    css::build_css,
    header::Header,
    transformer::{
        WithTransformer,
        code_block::CodeHighlightTransformer,
        epigraph::EpigraphTransformer,
        footnote::FootnoteTransformer,
        heading::HeadingDemoterTransformer,
        image::ImageCaptionTransformer,
        math::MathTransformer,
        toc::{TocTransformer, escape_attr},
    },
};
use tower_http::services::ServeDir;
use tower_livereload::LiveReloadLayer;
use walkdir::{DirEntry, WalkDir};

const INPUT_DIR: &str = "contents";
const OUPTPUT_DIR: &str = "public";
const POSTS_DIR: &str = "posts";
const TAGS_DIR: &str = "tags";

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    if env::args().any(|a| a == "serve") {
        serve().await?;
    } else {
        build_site()?;
    }

    Ok(())
}

async fn serve() -> color_eyre::Result<()> {
    // Initial build
    println!("Building site...");
    build_site()?;

    let current_dir = current_dir().with_note(|| "While getting the current working directory")?;
    let public_dir = current_dir.join(OUPTPUT_DIR);
    let contents_dir = current_dir.join(INPUT_DIR);
    let css_src = current_dir.join("style.css");

    // Setup live reload
    let livereload = LiveReloadLayer::new();
    let reloader = livereload.reloader();

    // Setup file watcher
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(event) => {
                // Ignore Access events (triggered when reading files) to
                // prevent infinite loops
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }

                println!("Change detected, rebuilding...");
                // We ignore build errors during watch mode to keep the server
                // alive
                if let Err(e) = build_site() {
                    eprintln!("Build failed: {}", e);
                } else {
                    println!("Rebuild complete.");
                    reloader.reload();
                }
            }
            Err(e) => eprintln!("Watch error: {}", e),
        }
    })?;

    // Watch contents directory and the style.css file
    watcher.watch(&contents_dir, RecursiveMode::Recursive)?;
    if css_src.exists() {
        watcher.watch(&css_src, RecursiveMode::NonRecursive)?;
    }

    // Setup Axum router
    let app = Router::new()
        .fallback_service(ServeDir::new(public_dir))
        .layer(livereload);

    println!("Serving on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_site() -> color_eyre::Result<()> {
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

    // Parse all the documents.
    let mut options = Options::empty();
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_SUPERSCRIPT);
    options.insert(Options::ENABLE_SUBSCRIPT);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let head =
        read_to_string(current_dir.join("header").with_extension("html")).unwrap_or_default();

    let footer =
        read_to_string(current_dir.join("footer").with_extension("html")).unwrap_or_default();

    let mut articles: Vec<Article> = Vec::new();

    source_documents
        .into_iter()
        .filter_map(|(entry, content)| {
            parse_item(
                entry,
                content,
                &input_dir,
                &output_dir,
                options,
                &mut articles,
            )
        })
        .for_each(|(out_path, rendered, header_html, body_header)| {
            // Build the actual page
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
{footer}
</html>
"#
            );
            _ = fs::write(out_path, html);
        });

    // Sort by time first, then title
    articles.sort_by(|a, b| b.ctime.cmp(&a.ctime).then_with(|| a.title.cmp(&b.title)));
    build_index(&output_dir, &articles, &head).with_note(|| "While building main index.")?;
    build_tag_indices(&articles, &output_dir, &head).with_note(|| "While building tag indices")?;

    // Minify and copy over style.css
    let stylesheet_in_path = current_dir.join("style").with_extension("css");
    // Ensure input stylesheet exists, otherwise skip (to avoid errors in fresh checkout)
    if stylesheet_in_path.exists() {
        let stylesheet_out_path = output_dir.join("style").with_extension("css");
        let stylesheet = build_css(stylesheet_in_path.as_path())?;
        fs::write(stylesheet_out_path, stylesheet)?;
    }

    Ok(())
}

fn parse_item(
    entry: DirEntry,
    content: String,
    input_dir: &Path,
    output_dir: &Path,
    options: Options,
    articles: &mut Vec<Article>,
) -> Option<(
    /* out path */ PathBuf,
    /* rendered */ String,
    /* header_html */ String,
    /* body header */ String,
)> {
    let rel_src = entry.path().strip_prefix(input_dir).ok()?.to_owned();

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
    let body_header = header.generate_body_head(&prefix);

    let parser = Parser::new_ext(content.as_str(), options);
    let events: Vec<Event<'_>> = parser.collect();

    let has_math = events
        .iter()
        .any(|e| matches!(e, Event::InlineMath(_) | Event::DisplayMath(_)));

    let katex_href = format!("{prefix}assets/katex/katex.min.css");
    let head_fragment = header.to_html(&css_href, has_math, &katex_href);

    // Apply transformers
    let transformed = events
        .into_iter()
        .with_transformer::<EpigraphTransformer<'_>>()
        .with_transformer::<CodeHighlightTransformer<'_, _>>()
        .with_transformer::<MathTransformer<'_, _>>()
        .with_transformer::<FootnoteTransformer<'_>>()
        .with_transformer::<HeadingDemoterTransformer<'_, _>>()
        .with_transformer::<TocTransformer<'_>>()
        .with_transformer::<ImageCaptionTransformer<_>>();

    let mut rendered = String::new();
    pulldown_cmark::html::push_html(&mut rendered, transformed);

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
}

fn build_index(output_dir: &Path, articles: &[Article], head: &str) -> io::Result<()> {
    let index_rel = std::path::Path::new("index.html");
    let index_prefix = prefix_to_root(index_rel);

    let index_html = render_listing_page("Index", "Index", articles, head, &index_prefix);

    fs::write(output_dir.join("index.html"), index_html)
}

fn build_tag_indices(articles: &[Article], output_dir: &Path, head: &str) -> io::Result<()> {
    let mut by_tag: BTreeMap<String, Vec<Article>> = BTreeMap::new();
    for a in articles {
        for t in &a.tags {
            by_tag.entry(t.clone()).or_default().push(a.clone());
        }
    }

    let tags_dir = output_dir.join(TAGS_DIR);
    fs::create_dir_all(&tags_dir)?;
    for (tag, tagged) in by_tag {
        let tag_rel = std::path::PathBuf::from(TAGS_DIR).join(format!("{tag}.html"));
        let tag_prefix = prefix_to_root(&tag_rel);

        let html = render_listing_page(
            &format!("Tag: {tag}"),
            &format!("Tag: {tag}"),
            &tagged,
            head,
            &tag_prefix,
        );

        fs::write(output_dir.join(tag_rel), html)?;
    }

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
