use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use brotli::CompressorWriter;
use color_eyre::{Section, eyre::eyre};
use flate2::{Compression, write::GzEncoder};
use minify_html::{Cfg, minify};
use pulldown_cmark::{Event, Options, Parser};
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    article::{render_listing_page, Article},
    config::{site_meta, SiteMeta, INPUT_DIR, OUTPUT_DIR, POSTS_DIR, TAGS_DIR},
    css::build_css,
    feed::write_feeds,
    header::{generic_og_meta, Header},
    templates::page_shell,
    transformer::{
        code_block::CodeHighlightTransformer, epigraph::EpigraphTransformer,
        footnote::{FootnoteTransformer, PlainFootnoteTransformer},
        heading::HeadingDemoterTransformer, image::ImageCaptionTransformer, math::MathTransformer,
        toc::TocTransformer, WithTransformer,
    },
    types::{Href, RelPath, Tag},
    utils::{escape_attr, prefix_to_root},
};

type ParsedDoc = (PathBuf, String);
struct RenderedPage {
    out_path: PathBuf,
    minified: Vec<u8>,
}

type RenderOutcome = (Vec<RenderedPage>, Vec<Article>);

/// Build once into OUTPUT_DIR using current working directory.
pub fn build_once() -> color_eyre::Result<()> {
    let root =
        std::env::current_dir().with_note(|| "While getting the current working directory")?;
    build_at(&root)
}

pub fn build_at(root: &Path) -> color_eyre::Result<()> {
    let ctx = BuildCtx::load_at(root)?;
    fs::create_dir_all(&ctx.output_dir)?;

    Pipeline::new(ctx)
        .discover()?
        .parse()?
        .transform()?
        .render()?
        .emit()
}

struct BuildCtx {
    current_dir: PathBuf,
    input_dir: PathBuf,
    output_dir: PathBuf,
    head_html: String,
    footer_html: String,
    site_meta: SiteMeta,
    parser_options: Options,
    min_cfg: Cfg,
}

impl BuildCtx {
    fn load_at(root: &Path) -> color_eyre::Result<Self> {
        let current_dir = root.to_path_buf();
        let input_dir = current_dir.join(INPUT_DIR);
        let output_dir = current_dir.join(OUTPUT_DIR);

        let head_html = fs::read_to_string(current_dir.join("header").with_extension("html"))
            .unwrap_or_default();
        let footer_html = fs::read_to_string(current_dir.join("footer").with_extension("html"))
            .unwrap_or_default();
        let site_meta = site_meta();

        let mut options = Options::empty();
        options.insert(Options::ENABLE_GFM);
        options.insert(Options::ENABLE_MATH);
        options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_SUPERSCRIPT);
        options.insert(Options::ENABLE_SUBSCRIPT);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);

        let mut min_cfg = Cfg::new();
        // Keep HTML minification aggressive, but leave CSS minification to
        // lightningcss (or external pipelines) to avoid double-processing.
        min_cfg.minify_css = false;
        min_cfg.minify_js = true;
        min_cfg.allow_optimal_entities = true;
        min_cfg.allow_noncompliant_unquoted_attribute_values = true;
        min_cfg.allow_removing_spaces_between_attributes = true;
        min_cfg.minify_doctype = true;
        min_cfg.remove_bangs = true;
        min_cfg.remove_processing_instructions = true;
        min_cfg.keep_closing_tags = false;
        min_cfg.keep_comments = false;
        min_cfg.keep_html_and_head_opening_tags = false;

        Ok(Self {
            current_dir,
            input_dir,
            output_dir,
            head_html,
            footer_html,
            site_meta,
            parser_options: options,
            min_cfg,
        })
    }
}

fn discover_sources(ctx: &BuildCtx) -> color_eyre::Result<Vec<(PathBuf, String)>> {
    let md_paths: Vec<PathBuf> = WalkDir::new(&ctx.input_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "md")
        })
        .map(|entry| entry.path().to_path_buf())
        .collect();

    let docs_res: Vec<_> = md_paths
        .par_iter()
        .map(|path| {
            fs::read_to_string(path)
                .map(|content| (path.clone(), content))
                .map_err(|e| eyre!("Failed to read {}: {e}", path.display()))
        })
        .collect();

    let mut docs: Vec<(PathBuf, String)> = docs_res.into_iter().collect::<Result<_, _>>()?;
    docs.par_sort_by(|a, b| a.0.cmp(&b.0));

    Ok(docs)
}
fn parse_sources(
    ctx: &BuildCtx,
    sources: Vec<(PathBuf, String)>,
) -> color_eyre::Result<Vec<ParsedDoc>> {
    let mut parsed = Vec::with_capacity(sources.len());
    for (full_path, content) in sources {
        let rel_src = full_path
            .strip_prefix(&ctx.input_dir)
            .map(|p| p.to_owned())
            .map_err(|_| eyre!("Path outside input_dir"))?;
        parsed.push((rel_src, content));
    }
    Ok(parsed)
}

fn transform_docs(parsed: Vec<ParsedDoc>) -> color_eyre::Result<Vec<ParsedDoc>> {
    Ok(parsed)
}

fn render_docs(ctx: &BuildCtx, items: Vec<ParsedDoc>) -> color_eyre::Result<RenderOutcome> {
    let results: Vec<_> = items
        .par_iter()
        .map(|(rel_src, content)| render_single(ctx, rel_src, content))
        .collect();

    let mut rendered_pages = Vec::with_capacity(results.len());
    let mut articles = Vec::with_capacity(results.len());

    for res in results {
        let (page, article) = res?;
        rendered_pages.push(page);
        articles.push(article);
    }

    // Sort by time first, then title
    articles.sort_by(|a, b| b.ctime.cmp(&a.ctime).then_with(|| a.title.cmp(&b.title)));

    Ok((rendered_pages, articles))
}

fn render_single(
    ctx: &BuildCtx,
    rel_src: &PathBuf,
    content: &str,
) -> color_eyre::Result<(RenderedPage, Article)> {
    let rel_out = PathBuf::from(POSTS_DIR)
        .join(rel_src)
        .with_extension("html");
    let rel_out = RelPath::new(rel_out).ok_or_else(|| eyre!("Output path must be relative"))?;
    let out_path = ctx.output_dir.join(rel_out.as_path());

    let href = Href::from_rel(&rel_out);
    let prefix = prefix_to_root(rel_out.as_path());
    let css_href = format!("{prefix}style.css");
    let page_url = format!("{}/{}", ctx.site_meta.base_url, href.as_str());

    let header = Header::try_from(content).unwrap_or_default();
    let body_header = header.generate_body_head(&prefix);

    let parser = Parser::new_ext(content, ctx.parser_options);
    let events: Vec<Event<'_>> = parser.collect();

    let has_math = events
        .iter()
        .any(|e| matches!(e, Event::InlineMath(_) | Event::DisplayMath(_)));

    let katex_href = format!("{prefix}assets/katex/katex.min.css");
    let mut head_fragment = header.to_html(&css_href, has_math, &katex_href);
    head_fragment.push_str(&header.opengraph_meta(&page_url, &ctx.site_meta));

    let page_body = render_page_body(events.clone());
    let feed_body = render_feed_body(events);

    // Capture the rendered article body (including header) for full-text feeds before adding
    // any extra navigation links that are only relevant on-page.
    let feed_content_html = format!("{body_header}{feed_body}");

    let mut page_body_with_nav = page_body;
    page_body_with_nav.push_str(&format!(
        r#"
<p class="meta"><a href="{0}index.html">Index</a></p>
"#,
        escape_attr(&prefix)
    ));

    let title = header
        .title()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| rel_out.as_path().to_string_lossy().to_string());

    let summary = header.description().map(ToOwned::to_owned);

    let article = Article {
        title,
        ctime: header.ctime(),
        updated: header.mtime(),
        summary,
        content_html: feed_content_html,
        href,
        tags: header.tags().0,
    };

    let page_html = page_shell(
        &ctx.head_html,
        &head_fragment,
        &body_header,
        &page_body_with_nav,
        &ctx.footer_html,
    );
    let minified = minify(page_html.as_bytes(), &ctx.min_cfg);

    Ok((
        RenderedPage {
            out_path,
            minified,
        },
        article,
    ))
}

fn render_page_body<'a>(events: Vec<Event<'a>>) -> String {
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
    rendered
}

fn render_feed_body<'a>(events: Vec<Event<'a>>) -> String {
    let transformed = events
        .into_iter()
        .with_transformer::<EpigraphTransformer<'_>>()
        .with_transformer::<CodeHighlightTransformer<'_, _>>()
        .with_transformer::<MathTransformer<'_, _>>()
        .with_transformer::<PlainFootnoteTransformer<'_>>()
        .with_transformer::<HeadingDemoterTransformer<'_, _>>()
        .with_transformer::<TocTransformer<'_>>()
        .with_transformer::<ImageCaptionTransformer<_>>();

    let mut rendered = String::new();
    pulldown_cmark::html::push_html(&mut rendered, transformed);
    rendered
}

fn emit_docs(
    ctx: &BuildCtx,
    rendered: Vec<RenderedPage>,
    articles: &[Article],
) -> color_eyre::Result<()> {
    for RenderedPage { out_path, minified } in rendered {
        write_with_compression(&out_path, &minified)?;
    }

    // Index and tag pages
    build_index(ctx, articles)?;
    build_tag_indices(ctx, articles)?;

    // Feeds; compress after writing
    write_feeds(&ctx.output_dir, articles)?;
    compress_existing(&ctx.output_dir.join("rss.xml"))?;
    compress_existing(&ctx.output_dir.join("atom.xml"))?;

    // Minify and copy over style.css, then compress
    let stylesheet_in_path = ctx.current_dir.join("style").with_extension("css");
    if stylesheet_in_path.exists() {
        let stylesheet_out_path = ctx.output_dir.join("style").with_extension("css");
        let stylesheet = build_css(stylesheet_in_path.as_path())?;
        write_with_compression(&stylesheet_out_path, stylesheet.as_bytes())?;
    }

    Ok(())
}

fn write_gzip_variant(path: &Path, data: &[u8]) -> io::Result<()> {
    let out_path = path.with_file_name(format!(
        "{}.gz",
        path.file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_default()
    ));

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    let compressed = encoder.finish()?;
    fs::write(out_path, compressed)
}

fn write_brotli_variant(path: &Path, data: &[u8]) -> io::Result<()> {
    let out_path = path.with_file_name(format!(
        "{}.br",
        path.file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_default()
    ));

    // q6 keeps strong compression while avoiding the very slow q11 default.
    let mut writer = CompressorWriter::new(Vec::new(), 4096, 6, 22);
    writer.write_all(data)?;
    let compressed = writer.into_inner();
    fs::write(out_path, compressed)
}

fn write_with_compression(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, data)?;
    write_gzip_variant(path, data)?;
    write_brotli_variant(path, data)?;
    Ok(())
}

fn compress_existing(path: &Path) -> io::Result<()> {
    let data = fs::read(path)?;
    write_gzip_variant(path, &data)?;
    write_brotli_variant(path, &data)?;
    Ok(())
}

fn build_index(ctx: &BuildCtx, articles: &[Article]) -> io::Result<()> {
    let index_rel = std::path::Path::new("index.html");
    let index_prefix = prefix_to_root(index_rel);
    let page_url = format!("{}/index.html", ctx.site_meta.base_url);

    let mut head_includes = String::new();
    head_includes.push_str(&ctx.head_html);
    head_includes.push_str(&format!(
        r#"
<meta name="description" content="{}">"#,
        escape_attr(&ctx.site_meta.description)
    ));
    head_includes.push_str(&generic_og_meta(
        "Index",
        &ctx.site_meta.description,
        &page_url,
        &ctx.site_meta,
        None,
    ));

    let index_html = render_listing_page("Index", "Index", articles, &head_includes, &index_prefix);

    let bytes = minify(index_html.as_bytes(), &ctx.min_cfg);
    write_with_compression(&ctx.output_dir.join("index.html"), &bytes)
}

trait PipelineStage {}
/// Pipeline typestate driver
struct Pipeline<S: PipelineStage> {
    ctx: BuildCtx,
    state: S,
}

// initial state
impl Pipeline<()> {
    fn new(ctx: BuildCtx) -> Self {
        Self { ctx, state: () }
    }

    fn discover(self) -> color_eyre::Result<Pipeline<Discovered>> {
        let docs = discover_sources(&self.ctx)?;
        Ok(Pipeline {
            ctx: self.ctx,
            state: Discovered(docs),
        })
    }
}

struct Discovered(Vec<(PathBuf, String)>);
impl PipelineStage for Discovered {}
struct Parsed(Vec<ParsedDoc>);
impl PipelineStage for Parsed {}
struct Transformed(Vec<ParsedDoc>);
impl PipelineStage for Transformed {}
struct Rendered {
    pages: Vec<RenderedPage>,
    articles: Vec<Article>,
}
impl PipelineStage for Rendered {}
impl PipelineStage for () {}

impl Pipeline<Discovered> {
    fn parse(self) -> color_eyre::Result<Pipeline<Parsed>> {
        let parsed = parse_sources(&self.ctx, self.state.0)?;
        Ok(Pipeline {
            ctx: self.ctx,
            state: Parsed(parsed),
        })
    }
}

impl Pipeline<Parsed> {
    fn transform(self) -> color_eyre::Result<Pipeline<Transformed>> {
        let transformed = transform_docs(self.state.0)?;
        Ok(Pipeline {
            ctx: self.ctx,
            state: Transformed(transformed),
        })
    }
}

impl Pipeline<Transformed> {
    fn render(self) -> color_eyre::Result<Pipeline<Rendered>> {
        let (pages, articles) = render_docs(&self.ctx, self.state.0)?;
        Ok(Pipeline {
            ctx: self.ctx,
            state: Rendered { pages, articles },
        })
    }
}

impl Pipeline<Rendered> {
    fn emit(self) -> color_eyre::Result<()> {
        emit_docs(&self.ctx, self.state.pages, &self.state.articles)
    }
}

fn build_tag_indices(ctx: &BuildCtx, articles: &[Article]) -> io::Result<()> {
    let mut by_tag: BTreeMap<Tag, Vec<Article>> = BTreeMap::new();
    for a in articles {
        for t in &a.tags {
            by_tag.entry(t.clone()).or_default().push(a.clone());
        }
    }

    let tags_dir = ctx.output_dir.join(TAGS_DIR);
    fs::create_dir_all(&tags_dir)?;
    for (tag, tagged) in by_tag {
        let tag_rel = std::path::PathBuf::from(TAGS_DIR).join(format!("{tag}.html"));
        let tag_prefix = prefix_to_root(&tag_rel);
        let page_url = format!("{}/tags/{tag}.html", ctx.site_meta.base_url);
        let page_description = format!("Posts tagged {tag}");

        let mut head_includes = String::new();
        head_includes.push_str(&ctx.head_html);
        head_includes.push_str(&format!(
            r#"
<meta name="description" content="{}">"#,
            escape_attr(&page_description)
        ));
        head_includes.push_str(&generic_og_meta(
            &format!("Tag: {tag}"),
            &page_description,
            &page_url,
            &ctx.site_meta,
            None,
        ));

        let html = render_listing_page(
            &format!("Tag: {tag}"),
            &format!("Tag: {tag}"),
            &tagged,
            &head_includes,
            &tag_prefix,
        );

        let bytes = minify(html.as_bytes(), &ctx.min_cfg);
        write_with_compression(&ctx.output_dir.join(tag_rel), &bytes)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
