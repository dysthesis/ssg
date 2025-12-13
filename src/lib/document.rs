use std::path::PathBuf;

use pulldown_cmark::{Options, Parser};

pub struct Document {
    path: PathBuf,
    content: String,
}

impl Document {
    pub fn new(path: PathBuf, content: String) -> Self {
        Self { path, content }
    }

    pub fn parse(&self) -> String {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&self.content, options);
        let mut html_output = String::new();
        pulldown_cmark::html::push_html(&mut html_output, parser);
        html_output
    }
}
