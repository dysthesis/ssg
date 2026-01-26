use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use color_eyre::{Section, eyre::eyre};
use minify_html::{Cfg, minify};
use pulldown_cmark::{Event, Options, Parser};
use walkdir::WalkDir;

use crate::{
    article::{Article, render_listing_page},
    config::{INPUT_DIR, OUTPUT_DIR, POSTS_DIR, TAGS_DIR},
    css::build_css,
    header::Header,
    templates::page_shell,
    transformer::{
        WithTransformer, code_block::CodeHighlightTransformer, epigraph::EpigraphTransformer,
        footnote::FootnoteTransformer, heading::HeadingDemoterTransformer,
        image::ImageCaptionTransformer, math::MathTransformer, toc::TocTransformer,
    },
    types::{Href, RelPath, Tag},
    utils::{escape_attr, prefix_to_root},
};

type ParsedDoc = (PathBuf, String);
type RenderedPage = (PathBuf, String);
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

        let mut options = Options::empty();
        options.insert(Options::ENABLE_GFM);
        options.insert(Options::ENABLE_MATH);
        options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_SUPERSCRIPT);
        options.insert(Options::ENABLE_SUBSCRIPT);
        options.insert(Options::ENABLE_SMART_PUNCTUATION);

        Ok(Self {
            current_dir,
            input_dir,
            output_dir,
            head_html,
            footer_html,
            parser_options: options,
            min_cfg: Cfg::new(),
        })
    }
}

fn discover_sources(ctx: &BuildCtx) -> color_eyre::Result<Vec<(PathBuf, String)>> {
    let mut md_paths: Vec<PathBuf> = Vec::new();
    let mut walk_errors: Vec<walkdir::Error> = Vec::new();

    for item in WalkDir::new(&ctx.input_dir) {
        match item {
            Ok(entry) => {
                if entry.file_type().is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "md")
                {
                    md_paths.push(entry.path().to_path_buf());
                }
            }
            Err(e) => walk_errors.push(e),
        }
    }

    if !walk_errors.is_empty() {
        return Err(eyre!(
            "Failed to open some directory entries: {walk_errors:?}"
        ));
    }

    md_paths.sort();

    let mut docs: Vec<(PathBuf, String)> = Vec::with_capacity(md_paths.len());
    let mut file_errors: Vec<(PathBuf, std::io::Error)> = Vec::new();

    for path in md_paths {
        match fs::read_to_string(&path) {
            Ok(content) => docs.push((path, content)),
            Err(e) => file_errors.push((path, e)),
        }
    }

    if !file_errors.is_empty() {
        return Err(eyre!("Failed to open some files: {file_errors:?}"));
    }

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
    let mut articles: Vec<Article> = Vec::new();
    let mut rendered_pages = Vec::new();

    for (rel_src, content) in items {
        let rel_out = PathBuf::from(POSTS_DIR)
            .join(&rel_src)
            .with_extension("html");
        let rel_out = RelPath::new(rel_out).ok_or_else(|| eyre!("Output path must be relative"))?;
        let out_path = ctx.output_dir.join(rel_out.as_path());

        let href = Href::from_rel(&rel_out);
        let prefix = prefix_to_root(rel_out.as_path());
        let css_href = format!("{prefix}style.css");

        let header = Header::try_from(content.as_str()).unwrap_or_default();
        let body_header = header.generate_body_head(&prefix);

        let parser = Parser::new_ext(content.as_str(), ctx.parser_options);
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
            .unwrap_or_else(|| rel_out.as_path().to_string_lossy().to_string());

        let article = Article {
            title,
            ctime: header.ctime(),
            href,
            tags: header.tags().0,
        };
        articles.push(article);

        let page_html = page_shell(
            &ctx.head_html,
            &head_fragment,
            &body_header,
            &rendered,
            &ctx.footer_html,
        );
        rendered_pages.push((out_path, page_html));
    }

    // Sort by time first, then title
    articles.sort_by(|a, b| b.ctime.cmp(&a.ctime).then_with(|| a.title.cmp(&b.title)));

    Ok((rendered_pages, articles))
}

fn emit_docs(
    ctx: &BuildCtx,
    rendered: Vec<RenderedPage>,
    articles: &[Article],
) -> color_eyre::Result<()> {
    for (out_path, page_html) in rendered {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(out_path, minify(page_html.as_bytes(), &ctx.min_cfg))?;
    }

    build_index(ctx, articles)?;
    build_tag_indices(ctx, articles)?;

    // Minify and copy over style.css
    let stylesheet_in_path = ctx.current_dir.join("style").with_extension("css");
    if stylesheet_in_path.exists() {
        let stylesheet_out_path = ctx.output_dir.join("style").with_extension("css");
        let stylesheet = build_css(stylesheet_in_path.as_path())?;
        fs::write(stylesheet_out_path, stylesheet)?;
    }

    Ok(())
}

fn build_index(ctx: &BuildCtx, articles: &[Article]) -> io::Result<()> {
    let index_rel = std::path::Path::new("index.html");
    let index_prefix = prefix_to_root(index_rel);

    let index_html = render_listing_page("Index", "Index", articles, &ctx.head_html, &index_prefix);

    fs::write(
        ctx.output_dir.join("index.html"),
        minify(index_html.as_bytes(), &ctx.min_cfg),
    )
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

        let html = render_listing_page(
            &format!("Tag: {tag}"),
            &format!("Tag: {tag}"),
            &tagged,
            &ctx.head_html,
            &tag_prefix,
        );

        fs::write(
            ctx.output_dir.join(tag_rel),
            minify(html.as_bytes(), &ctx.min_cfg),
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
