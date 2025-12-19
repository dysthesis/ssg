//! This module models the life cycle of a Markdown document as it gets
//! converted into HTML. Note that the actual translation is not handled here;
//! this module instead parses it using pulldown_cmark, and passes it to a
//! `Renderer` to render the resulting `Event<'_>` to HTML.

use pulldown_cmark::{CowStr, Event, Options, Parser, html};
use std::{
    env::current_dir,
    fmt::Display,
    fs::{File, create_dir_all},
    io::{BufWriter, Write},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use crate::highlighter::{escape_html, syntect::SyntectHighlighter};
use crate::math::katex::{KATEX_STYLESHEET_LINK, KatexRenderer};
use crate::transformer::code_block::ToCodeBlockTransformer;
use crate::transformer::math::ToMathTransformer;

/// Which features to support when parsing the Markdown file.
const PARSE_OPTIONS: [Options; 5] = [
    Options::ENABLE_FOOTNOTES,
    Options::ENABLE_TABLES,
    Options::ENABLE_MATH,
    Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
    Options::ENABLE_STRIKETHROUGH,
];

const OUTPUT_ROOT: &str = "out";

pub fn output_root_path() -> PathBuf {
    std::env::var("SSG_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(OUTPUT_ROOT))
}

fn raw_html_allowed() -> bool {
    matches!(
        std::env::var("SSG_ALLOW_RAW_HTML"),
        Ok(val) if val == "1" || val.eq_ignore_ascii_case("true")
    )
}

fn normalise_relative_path(path: &Path) -> std::io::Result<PathBuf> {
    let mut normalised = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalised.pop() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "path attempts to escape output root",
                    ));
                }
            }
            Component::Normal(part) => normalised.push(part),
            Component::RootDir | Component::Prefix(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "absolute components not permitted in relative path",
                ))
            }
        }
    }
    Ok(normalised)
}

/// Compute the output path for a given input path relative to a working directory
/// without touching the filesystem, rejecting traversal outside the output root.
pub fn compute_output_path(input_path: &Path, working_dir: &Path) -> std::io::Result<PathBuf> {
    let relative_path = if input_path.is_absolute() {
        input_path
            .strip_prefix(working_dir)
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                input_path
                    .ancestors()
                    .last()
                    .and_then(|root| input_path.strip_prefix(root).ok())
                    .map(PathBuf::from)
            })
            .unwrap_or_else(|| {
                input_path
                    .strip_prefix(Path::new("/"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| input_path.to_path_buf())
            })
    } else {
        input_path.to_path_buf()
    };

    let safe_relative = normalise_relative_path(&relative_path)?;

    let output_root = output_root_path();
    let mut output_path = output_root.join(safe_relative);
    output_path.set_extension("html");
    Ok(output_path)
}

/// Process multiple documents in-memory without filesystem operations.
/// This is useful for benchmarking CPU-only performance.
#[cfg(feature = "bench")]
pub fn process_documents_in_memory(
    documents: &[(PathBuf, String)],
    stylesheet: Option<Arc<String>>,
) -> Vec<(PathBuf, String)> {
    documents
        .iter()
        .map(|(path, content)| {
            let doc = Document::new(path.clone(), content, stylesheet.clone());
            let parsed = doc.parse();
            let html_doc = parsed.build();

            let mut buffer = Vec::new();
            html_doc
                .write_to(&mut buffer)
                .expect("writing to memory should not fail");
            let html_string = String::from_utf8(buffer).expect("HTML should be valid UTF-8");

            (path.clone(), html_string)
        })
        .collect()
}

pub trait Parseable<T> {
    fn parse(self) -> T;
}

pub trait Buildable<T> {
    fn build(self) -> T;
}

pub trait Writeable {
    fn write(self) -> std::io::Result<()>;
}

/// A newtype for HTML to ensure that it does not get mixed up with a random
/// string.
pub struct Html(String);

impl Display for Html {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        write!(f, "{inner}")
    }
}

impl From<String> for Html {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Html {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Html {
    /// Get the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into the inner string
    pub fn into_string(self) -> String {
        self.0
    }

    /// Convert into a `CowStr` without additional allocation
    pub fn into_cow_str(self) -> pulldown_cmark::CowStr<'static> {
        pulldown_cmark::CowStr::from(self.0)
    }
}

/// A raw Markdown document with an associated stylesheet to style the resulting
/// HTML page.
pub struct Document<'a> {
    /// The path to the Markdown document.
    path: PathBuf,
    /// The contents of the Markdown document.
    content: &'a str,
    /// The CSS to style the resulting page with.
    stylesheet: Option<Arc<String>>,
}

impl<'a> Document<'a> {
    /// Construct a new document given the path, content, and stylesheet.
    pub fn new(path: PathBuf, content: &'a str, stylesheet: Option<Arc<String>>) -> Document<'a> {
        Document {
            path,
            content,
            stylesheet,
        }
    }
}

impl<'a> Parseable<ParsedDocument<'a, Parser<'a>>> for Document<'a> {
    /// Parse the document into events with pulldown_cmark.
    fn parse(self) -> ParsedDocument<'a, Parser<'a>> {
        // Add the enabled options to the list of options
        let mut options = Options::empty();
        PARSE_OPTIONS
            .into_iter()
            .for_each(|opt| options.insert(opt));

        // Construct a new pulldown iterator from the document.
        let iterator = Parser::new_ext(self.content, options);
        let path = self.path;
        let stylesheet = self.stylesheet;
        ParsedDocument {
            path,
            iterator,
            stylesheet,
        }
    }
}

/// A parsed Markdown document, with an associated event iterator.
pub struct ParsedDocument<'a, T>
where
    T: Iterator<Item = Event<'a>>,
{
    /// The path to the Markdown document.
    path: PathBuf,
    /// The event iterator of syntax elements from the original document.
    pub iterator: T,
    /// The CSS to style the resulting page with.
    stylesheet: Option<Arc<String>>,
}

impl<'a, T> Buildable<HtmlDocument> for ParsedDocument<'a, T>
where
    T: Iterator<Item = Event<'a>>,
{
    /// Consume the event iterator into an HTML body.
    fn build(self) -> HtmlDocument {
        // Collect events to break lifetime dependencies
        let allow_raw_html = raw_html_allowed();
        let mut events: Vec<Event<'static>> =
            self.iterator.map(|event| event.into_static()).collect();

        if !allow_raw_html {
            events = events
                .into_iter()
                .map(|event| match event {
                    Event::Html(html) => Event::Text(CowStr::from(html)),
                    Event::InlineHtml(html) => Event::Text(CowStr::from(html)),
                    other => other,
                })
                .collect();
        }

        let math_required = events.iter().any(|event| {
            matches!(event, Event::InlineMath(_) | Event::DisplayMath(_))
        });

        let highlighter = SyntectHighlighter::default();
        let math_renderer = KatexRenderer::new();

        let transformed = events
            .into_iter()
            .highlight_code(&highlighter)
            .render_math(&math_renderer);

        // Convert transformed events to HTML
        let mut html_output = String::new();
        html::push_html(&mut html_output, transformed);
        let content = Html::from(html_output);

        let path = self.path;
        let stylesheet = self.stylesheet;
        HtmlDocument {
            path,
            body: content,
            stylesheet,
            math_required,
        }
    }
}

/// An HTML document with the body rendered.
pub struct HtmlDocument {
    /// Path to the original Markdown document
    path: PathBuf,
    /// The body of the HTML document
    body: Html,
    /// The stylesheet to style the page with
    stylesheet: Option<Arc<String>>,
    /// Whether KaTeX assets are required
    math_required: bool,
}

impl HtmlDocument {
    /// Construct a new HtmlDocument directly (useful for benchmarking)
    #[cfg(feature = "bench")]
    pub fn new(
        path: PathBuf,
        body: Html,
        stylesheet: Option<Arc<String>>,
        math_required: bool,
    ) -> Self {
        HtmlDocument {
            path,
            body,
            stylesheet,
            math_required,
        }
    }

    /// Get a reference to the HTML body
    pub fn body(&self) -> &Html {
        &self.body
    }

    /// Get a reference to the path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get a reference to the stylesheet
    pub fn stylesheet(&self) -> Option<&str> {
        self.stylesheet.as_ref().map(|arc| arc.as_str())
    }

    pub fn math_required(&self) -> bool {
        self.math_required
    }

    /// Write the HTML document to an arbitrary writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let title = self
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(escape_html)
            .unwrap_or_else(|| String::from("Document"));

        let stylesheet_block = match &self.stylesheet {
            Some(css) => format!("  <style>\n{css}\n  </style>\n"),
            None => String::new(),
        };

        let katex_block = if self.math_required {
            let tag = std::env::var("SSG_KATEX_STYLESHEET")
                .map(|override_href| format!(r#"<link rel="stylesheet" href="{override_href}">"#))
                .unwrap_or_else(|_| KATEX_STYLESHEET_LINK.to_string());
            format!("  {tag}\n")
        } else {
            String::new()
        };

        write!(
            writer,
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>{title}</title>\n{katex}{stylesheet}</head>\n<body>\n{body}\n</body>\n</html>\n",
            title = title,
            katex = katex_block,
            stylesheet = stylesheet_block,
            body = self.body,
        )?;

        Ok(())
    }
}

impl Writeable for HtmlDocument {
    /// Construct a full HTML document and write it to `./out/{self.path}`.
    fn write(self) -> std::io::Result<()> {
        let cwd = current_dir()?;
        let output_path = compute_output_path(&self.path, &cwd)?;

        if let Some(parent) = output_path.parent() {
            create_dir_all(parent)?;
        }

        let mut writer = BufWriter::new(File::create(&output_path)?);
        self.write_to(&mut writer)?;
        writer.flush()
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::highlighter::escape_html;
    use crate::test_support::{DEFAULT_CASES, FILE_CASES, gen_any_utf8, gen_path};
    use proptest::prelude::*;
    use proptest::{collection, option};
    use pulldown_cmark::Tag;
    use std::{
        env::{current_dir, set_current_dir},
        fs,
        path::{Path, PathBuf},
    };
    use tempfile::TempDir;

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: DEFAULT_CASES,
            ..ProptestConfig::default()
        }
    }

    fn file_config() -> ProptestConfig {
        ProptestConfig {
            cases: FILE_CASES,
            ..ProptestConfig::default()
        }
    }

    struct CwdGuard {
        original: PathBuf,
    }

    impl CwdGuard {
        fn enter<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
            let original = current_dir()?;
            set_current_dir(path)?;
            Ok(Self { original })
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = set_current_dir(&self.original);
        }
    }

    fn extract_title(html: &str) -> Option<&str> {
        let start = html.find("<title>")?;
        let end = html[start..].find("</title>")?;
        let title_start = start + "<title>".len();
        Some(&html[title_start..start + end])
    }

    fn feature_markdown() -> impl Strategy<Value = String> {
        let fragments = [
            "[^1]\n\n[^1]: footnote here".to_string(),
            "|a|b|\n|---|---|\n|1|2|".to_string(),
            "Inline math $x+y$ and display:\n\n$$z=1$$".to_string(),
            "---\ntitle: demo\n---".to_string(),
            "This uses ~~strikethrough~~ text.".to_string(),
        ];
        collection::vec(
            prop_oneof![
                Just(fragments[0].clone()),
                Just(fragments[1].clone()),
                Just(fragments[2].clone()),
                Just(fragments[3].clone()),
                Just(fragments[4].clone()),
            ],
            1..=fragments.len(),
        )
        .prop_map(|parts| parts.join("\n\n"))
    }

    fn contains_feature_specific_event(events: &[Event<'_>]) -> bool {
        events.iter().any(|event| {
            matches!(
                event,
                Event::FootnoteReference(_)
                    | Event::Start(Tag::FootnoteDefinition(_))
                    | Event::Start(Tag::Table(_))
                    | Event::Start(Tag::TableHead)
                    | Event::Start(Tag::TableRow)
                    | Event::Start(Tag::TableCell)
                    | Event::Start(Tag::Strikethrough)
                    | Event::InlineMath(_)
                    | Event::DisplayMath(_)
                    | Event::Start(Tag::MetadataBlock(_))
            )
        })
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn parse_applies_options(md in feature_markdown()) {
            let doc = Document::new(PathBuf::from("sample.md"), &md, None);
            let ParsedDocument { iterator, .. } = doc.parse();
            let events: Vec<_> = iterator.collect();
            prop_assert!(contains_feature_specific_event(&events));
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn parse_does_not_panic_on_arbitrary_utf8(input in gen_any_utf8()) {
            let doc = Document::new(PathBuf::from("input.md"), &input, None);
            let ParsedDocument { iterator, .. } = doc.parse();
            let _: Vec<_> = iterator.collect();
        }
    }

    proptest! {
        #![proptest_config(file_config())]
        #[test]
        fn html_write_respects_structure(path in gen_path(), body in gen_any_utf8(), stylesheet in option::of(gen_any_utf8())) {
            let temp = TempDir::new().expect("tempdir");
            let _guard = CwdGuard::enter(temp.path()).expect("change cwd");

            let doc = HtmlDocument {
                path: path.clone(),
                body: Html::from(body.clone()),
                stylesheet: stylesheet.clone().map(Arc::new),
                math_required: false,
            };
            doc.write().expect("write should succeed");

            let output_path = compute_output_path(&path, temp.path()).expect("output path");
            prop_assert_eq!(output_path.extension().and_then(|ext| ext.to_str()), Some("html"));

            let html = fs::read_to_string(&output_path).expect("read written file");
            prop_assert_eq!(html.lines().next(), Some("<!DOCTYPE html>"));
            prop_assert_eq!(html.match_indices("<html").count(), 1);
            prop_assert_eq!(html.match_indices("<head>").count(), 1);
            prop_assert_eq!(html.match_indices("<body>").count(), 1);
            prop_assert_eq!(html.match_indices(KATEX_STYLESHEET_LINK).count(), 0);

            let body_start = html.find("<body>\n").expect("body start") + "<body>\n".len();
            let body_end = html.rfind("\n</body>").expect("body end");
            let body_slice = &html[body_start..body_end];
            prop_assert_eq!(body_slice, body.as_str());

            let title = extract_title(&html).expect("title present");
            let expected_title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(escape_html)
                .unwrap_or_else(|| "Document".to_string());
            prop_assert_eq!(title, expected_title);
            prop_assert!(!title.contains('<'));
            prop_assert!(!title.contains('>'));
            prop_assert!(!title.contains('"'));
            prop_assert!(!title.contains('\''));

            match stylesheet {
                Some(css) => {
                    let expected_block = format!("  <style>\n{css}\n  </style>\n");
                    prop_assert_eq!(html.match_indices("<style>").count(), 1);
                    prop_assert_eq!(html.match_indices("</style>").count(), 1);
                    prop_assert_eq!(html.match_indices(&expected_block).count(), 1);
                }
                None => {
                    prop_assert_eq!(html.match_indices("<style>").count(), 0);
                }
            }
        }
    }

    #[test]
    fn compute_output_path_rejects_traversal_outside_output_root() {
        let cwd = Path::new("/workspace");
        let bad = Path::new("../escape.md");
        let result = compute_output_path(bad, cwd);
        assert!(result.is_err());
    }

    #[test]
    fn raw_html_is_escaped_by_default() {
        // Environment mutation is unsafe on some platforms; keep scoped to this test.
        unsafe { std::env::remove_var("SSG_ALLOW_RAW_HTML") };
        let md = "<div>pwn</div>";

        let doc = Document::new(PathBuf::from("note.md"), md, None);
        let html = doc.parse().build();
        let body = html.body().as_str();

        assert!(body.contains("&lt;div"));
        assert!(!body.contains("<div>pwn</div>"));
    }

    #[test]
    fn katex_stylesheet_emitted_only_for_math() {
        let without_math = Document::new(PathBuf::from("plain.md"), "hello", None)
            .parse()
            .build();
        let mut buf = Vec::new();
        without_math.write_to(&mut buf).unwrap();
        let html = String::from_utf8(buf).unwrap();
        assert_eq!(html.matches(KATEX_STYLESHEET_LINK).count(), 0);

        let with_math =
            Document::new(PathBuf::from("math.md"), "inline $a+b$", None).parse().build();
        let mut buf_math = Vec::new();
        with_math.write_to(&mut buf_math).unwrap();
        let html_math = String::from_utf8(buf_math).unwrap();
        assert_eq!(html_math.matches(KATEX_STYLESHEET_LINK).count(), 1);
    }
}
