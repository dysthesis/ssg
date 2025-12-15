//! This module models the life cycle of a Markdown document as it gets
//! converted into HTML. Note that the actual translation is not handled here;
//! this module instead parses it using pulldown_cmark, and passes it to a
//! `Renderer` to render the resulting `Event<'_>` to HTML.

use pulldown_cmark::{Event, Options, Parser};
use std::{
    env::current_dir,
    fmt::Display,
    fs::{File, create_dir_all},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use crate::renderer::katex::KATEX_STYLESHEET_LINK;
use crate::renderer::{Renderer, escape_html};

/// Which features to support when parsing the Markdown file.
const PARSE_OPTIONS: [Options; 5] = [
    Options::ENABLE_FOOTNOTES,
    Options::ENABLE_TABLES,
    Options::ENABLE_MATH,
    Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
    Options::ENABLE_STRIKETHROUGH,
];

const OUTPUT_ROOT: &str = "out";

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

/// A raw Markdown document with an associated stylesheet to style the resulting
/// HTML page.
pub struct Document<'a> {
    /// The path to the Markdown document.
    path: PathBuf,
    /// The contents of the Markdown document.
    content: &'a str,
    /// The CSS to style the resulting page with.
    stylesheet: Option<String>,
}

impl<'a> Document<'a> {
    /// Construct a new document given the path, content, and stylesheet.
    pub fn new(path: PathBuf, content: &'a str, stylesheet: Option<String>) -> Document<'a> {
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
    iterator: T,
    /// The CSS to style the resulting page with.
    stylesheet: Option<String>,
}

impl<'a, T> Buildable<HtmlDocument> for ParsedDocument<'a, T>
where
    T: Iterator<Item = Event<'a>>,
{
    /// Consume the event iterator into an HTML body.
    fn build(self) -> HtmlDocument {
        // Construct a default renderer...
        let renderer = Renderer::default();
        // ...and consume the associated event iterator
        let content = renderer.render(self.iterator);

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
    stylesheet: Option<String>,
}

impl Writeable for HtmlDocument {
    /// Construct a full HTML document and write it to `./out/{self.path}`.
    fn write(self) -> std::io::Result<()> {
        let HtmlDocument {
            path,
            body: content,
            stylesheet,
        } = self;

        // Figure out where the path should live
        let cwd = current_dir()?;
        let relative_path = if path.is_absolute() {
            path.strip_prefix(&cwd)
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    path.ancestors()
                        .last()
                        .and_then(|root| path.strip_prefix(root).ok())
                        .map(PathBuf::from)
                })
                .unwrap_or_else(|| path.clone())
        } else {
            path.clone()
        };

        let mut output_path = Path::new(OUTPUT_ROOT).join(relative_path);
        output_path.set_extension("html");

        if let Some(parent) = output_path.parent() {
            create_dir_all(parent)?;
        }

        let title = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(escape_html)
            .unwrap_or_else(|| String::from("Document"));

        let mut writer = BufWriter::new(File::create(&output_path)?);

        let stylesheet_block = match &stylesheet {
            Some(css) => format!("  <style>\n{css}\n  </style>\n"),
            None => String::new(),
        };

        // Write the rendered HTML page
        write!(
            writer,
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>{title}</title>\n  {katex}\n{stylesheet}</head>\n<body>\n{body}\n</body>\n</html>\n",
            title = title,
            katex = KATEX_STYLESHEET_LINK,
            stylesheet = stylesheet_block,
            body = content,
        )?;

        writer.flush()
    }
}
