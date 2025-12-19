//! This module models the life cycle of a Markdown document as it gets
//! converted into HTML. Note that the actual translation is not handled here;
//! this module instead parses it using pulldown_cmark, and passes it to a
//! `Renderer` to render the resulting `Event<'_>` to HTML.

use pulldown_cmark::{Event, Options, Parser, html};
use std::{
    env::current_dir,
    fmt::Display,
    fs::{File, create_dir_all},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
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

/// Compute the output path for a given input path relative to a working directory
/// without touching the filesystem.
pub fn compute_output_path(input_path: &Path, working_dir: &Path) -> PathBuf {
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
            .unwrap_or_else(|| input_path.to_path_buf())
    } else {
        input_path.to_path_buf()
    };

    let mut output_path = Path::new(OUTPUT_ROOT).join(relative_path);
    output_path.set_extension("html");
    output_path
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
        let events: Vec<Event<'a>> = self.iterator.map(|event| event.into_static()).collect();

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
}

impl HtmlDocument {
    /// Construct a new HtmlDocument directly (useful for benchmarking)
    #[cfg(feature = "bench")]
    pub fn new(path: PathBuf, body: Html, stylesheet: Option<Arc<String>>) -> Self {
        HtmlDocument {
            path,
            body,
            stylesheet,
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

        write!(
            writer,
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>{title}</title>\n  {katex}\n{stylesheet}</head>\n<body>\n{body}\n</body>\n</html>\n",
            title = title,
            katex = KATEX_STYLESHEET_LINK,
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
        let output_path = compute_output_path(&self.path, &cwd);

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

    fn expected_output_path(cwd: &Path, path: &Path) -> PathBuf {
        let relative_path = if path.is_absolute() {
            path.strip_prefix(cwd)
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    path.ancestors()
                        .last()
                        .and_then(|root| path.strip_prefix(root).ok())
                        .map(PathBuf::from)
                })
                .unwrap_or_else(|| path.to_path_buf())
        } else {
            path.to_path_buf()
        };

        let mut output_path = Path::new(super::OUTPUT_ROOT).join(relative_path);
        output_path.set_extension("html");
        output_path
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
            };
            doc.write().expect("write should succeed");

            let output_path = expected_output_path(temp.path(), &path);
            prop_assert_eq!(output_path.extension().and_then(|ext| ext.to_str()), Some("html"));

            let html = fs::read_to_string(&output_path).expect("read written file");
            prop_assert_eq!(html.lines().next(), Some("<!DOCTYPE html>"));
            prop_assert_eq!(html.match_indices("<html").count(), 1);
            prop_assert_eq!(html.match_indices("<head>").count(), 1);
            prop_assert_eq!(html.match_indices("<body>").count(), 1);
            prop_assert_eq!(html.match_indices(KATEX_STYLESHEET_LINK).count(), 1);

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
}
