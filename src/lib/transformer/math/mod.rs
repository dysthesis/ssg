use katex::Opts;
use pulldown_cmark::{CowStr, Event};

use crate::transformer::Transformer;

/// Render math expressions via KaTeX.
pub struct MathTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    inner: I,
}

impl<'a, I> Iterator for MathTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Event::InlineMath(source) => {
                let html = render_math(source.as_ref(), false);
                Some(Event::InlineHtml(CowStr::from(html)))
            }
            Event::DisplayMath(source) => {
                let html = render_math(source.as_ref(), true);
                Some(Event::Html(CowStr::from(html)))
            }
            other => Some(other),
        }
    }
}

fn render_math(source: &str, display_mode: bool) -> String {
    let mut builder = Opts::builder();
    builder.display_mode(display_mode);

    let opts = builder.build().unwrap_or_default();

    match katex::render_with_opts(source, &opts) {
        Ok(res) => res,
        Err(_) => source.to_string(),
    }
}

impl<'a, I> Transformer<'a, I> for MathTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn transform(inner: I) -> Self {
        Self { inner }
    }
}

#[cfg(test)]
mod tests;
