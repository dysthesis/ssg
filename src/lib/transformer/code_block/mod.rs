use std::{
    io::{BufReader, Cursor},
    sync::OnceLock,
};

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag, TagEnd};
use syntect::{
    highlighting::ThemeSet,
    html::highlighted_html_for_string,
    parsing::{SyntaxReference, SyntaxSet},
};

use crate::{transformer::Transformer, utils::escape_html};

/// An enum to keep track of the state of the highlighter in the code block.
pub enum CodeBlockState<'a> {
    /// Not in code block, pass through the event as-is.
    Passthrough,
    /// Currently inside a code block of language `lang`, so we accumulate all
    /// events until an `Event::End(TagEnd::CodeBlock)` is reached.
    Accumulating { lang: CodeBlockKind<'a> },
}

/// A transformer to highlight code blocks
pub struct CodeHighlightTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    /// The inner iterator which CodeBlockTransformer transforms
    inner: I,
    /// Buffer to accumulate any code.
    buffer: String,
    /// Current state of the transformer; are we inside a code block?
    state: CodeBlockState<'a>,
}

impl<'a, I> Iterator for CodeHighlightTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // We may want to accumulate multiple events to construct one
        // transformed event; that is, we want to collect everything between
        // Event::Star(Tag::CodeBlock(lang)) to
        // Event::End(TagEnd::CodeBlock(lang)), to generate a single HTML event,
        // so we loop.
        loop {
            let event = self.inner.next()?;
            match &self.state {
                CodeBlockState::Passthrough => match event {
                    Event::Start(Tag::CodeBlock(lang)) => {
                        // Transition to accumulating state
                        self.state = CodeBlockState::Accumulating { lang };
                        self.buffer.clear();
                        // Don't return anything -- swallow the Start event
                        continue;
                    }
                    // All other events pass through unchanged
                    other => return Some(other),
                },
                CodeBlockState::Accumulating { lang: _ } => match event {
                    Event::End(TagEnd::CodeBlock) => {
                        let CodeBlockState::Accumulating { lang } =
                            std::mem::replace(&mut self.state, CodeBlockState::Passthrough)
                        else {
                            unreachable!()
                        };

                        let language = match lang {
                            CodeBlockKind::Fenced(ref l) => Some(l.as_ref()),
                            CodeBlockKind::Indented => None,
                        };

                        let syntax_set = syntax_set();
                        let theme = theme();

                        let syntax: &SyntaxReference = language
                            .and_then(|lang| syntax_set.find_syntax_by_token(lang))
                            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

                        let rendered =
                            highlighted_html_for_string(&self.buffer, syntax_set, syntax, &theme)
                                .unwrap_or_else(|_| fallback_plain(&self.buffer, language));

                        return Some(Event::Html(CowStr::from(rendered)));
                    }
                    Event::Text(text) | Event::Code(text) => {
                        self.buffer.push_str(text.as_ref());
                        continue;
                    }
                    Event::SoftBreak | Event::HardBreak => {
                        self.buffer.push('\n');
                        continue;
                    }
                    Event::Html(html) | Event::InlineHtml(html) => {
                        self.buffer.push_str(html.as_ref());
                        continue;
                    }
                    Event::InlineMath(math) | Event::DisplayMath(math) => {
                        self.buffer.push_str(math.as_ref());
                        continue;
                    }
                    _ => continue,
                },
            }
        }
    }
}

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> syntect::highlighting::Theme {
    let raw_theme = include_bytes!("../../../../assets/theme.tmTheme");
    let cursor = Cursor::new(raw_theme);
    let mut reader = BufReader::new(cursor);
    ThemeSet::load_from_reader(&mut reader).unwrap_or_default()
}

/// Backup renderer in case syntect fails for whatever reason
pub fn fallback_plain(source: &str, language: Option<&str>) -> String {
    let mut out = String::with_capacity(source.len() + 32);
    out.push_str("<pre><code");
    if let Some(lang) = language {
        out.push_str(" class=\"language-");
        out.push_str(lang);
        out.push('"');
    }
    out.push('>');
    out.push_str(&escape_html(source));
    out.push_str("</code></pre>\n");
    out
}

impl<'a, I> Transformer<'a, I> for CodeHighlightTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn transform(inner: I) -> Self {
        Self {
            inner,
            buffer: String::new(),
            state: CodeBlockState::Passthrough,
        }
    }
}

#[cfg(test)]
mod tests;
