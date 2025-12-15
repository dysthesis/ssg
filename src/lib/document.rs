//! This module models the life cycle of a Markdown document as it gets
//! converted into HTML. Note that the actual translation is not handled here;
//! this module instead parses it using pulldown_cmark, and passes it to a
//! `Renderer` to render the resulting `Event<'_>` to HTML.

use pulldown_cmark::{Event, Options, Parser};
use std::{fmt::Display, path::PathBuf};

const PARSE_OPTIONS: [Options; 5] = [
    Options::ENABLE_FOOTNOTES,
    Options::ENABLE_TABLES,
    Options::ENABLE_MATH,
    Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
    Options::ENABLE_STRIKETHROUGH,
];

pub trait Parseable<T> {
    fn parse(self) -> T;
}

pub trait Buildable<T> {
    fn build(self) -> T;
}

pub trait Writeable {
    fn write(self) -> std::io::Result<()>;
}

pub struct Html(String);

impl Display for Html {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        write!(f, "{inner}")
    }
}

pub struct Document<'a> {
    path: PathBuf,
    content: &'a str,
}

impl<'a> Document<'a> {
    pub fn new(path: PathBuf, content: &'a str) -> Document<'a> {
        Document { path, content }
    }
}

impl<'a> Parseable<ParsedDocument<'a, Parser<'a>>> for Document<'a> {
    fn parse(self) -> ParsedDocument<'a, Parser<'a>> {
        let mut options = Options::empty();
        PARSE_OPTIONS
            .into_iter()
            .for_each(|opt| options.insert(opt));

        let iterator = Parser::new_ext(self.content, options);
        let path = self.path;
        ParsedDocument { path, iterator }
    }
}

pub struct ParsedDocument<'a, T>
where
    T: Iterator<Item = Event<'a>>,
{
    path: PathBuf,
    iterator: T,
}

impl<'a, T> Buildable<HtmlDocument> for ParsedDocument<'a, T>
where
    T: Iterator<Item = Event<'a>>,
{
    fn build(self) -> HtmlDocument {
        todo!()
    }
}

pub struct HtmlDocument {
    path: PathBuf,
    content: String,
}

impl Writeable for HtmlDocument {
    fn write(self) -> std::io::Result<()> {
        todo!()
    }
}
