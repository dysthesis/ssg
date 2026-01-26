use std::{
    io::{BufReader, Cursor},
    sync::OnceLock,
};

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag, TagEnd};
use syntect::{
    highlighting::ThemeSet,
    html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style},
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

use crate::{
    transformer::Transformer,
    utils::{escape_attr, escape_html},
};

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

                        let syntax: &SyntaxReference = language
                            .and_then(|lang| syntax_set.find_syntax_by_token(lang))
                            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

                        let rendered =
                            render_classed_html(&self.buffer, syntax_set, syntax, language)
                                .unwrap_or_else(|| fallback_plain(&self.buffer, language));

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

static THEME: OnceLock<syntect::highlighting::Theme> = OnceLock::new();
fn theme() -> &'static syntect::highlighting::Theme {
    THEME.get_or_init(|| {
        let raw_theme = include_bytes!("../../../../assets/theme.tmTheme");
        let cursor = Cursor::new(raw_theme);
        let mut reader = BufReader::new(cursor);
        ThemeSet::load_from_reader(&mut reader).unwrap_or_default()
    })
}

static HIGHLIGHT_CSS: OnceLock<String> = OnceLock::new();
/// Return the CSS needed for class-based syntax highlighting.
pub fn highlight_css() -> &'static str {
    HIGHLIGHT_CSS.get_or_init(|| {
        css_for_theme_with_class_style(theme(), ClassStyle::Spaced).unwrap_or_default()
    })
}

fn render_classed_html(
    source: &str,
    syntax_set: &SyntaxSet,
    syntax: &SyntaxReference,
    language: Option<&str>,
) -> Option<String> {
    let mut generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, ClassStyle::Spaced);

    for line in LinesWithEndings::from(source) {
        generator
            .parse_html_for_line_which_includes_newline(line)
            .ok()?;
    }

    let mut out = String::with_capacity(source.len() + 48);
    out.push_str("<pre class=\"code");
    if let Some(lang) = language {
        out.push(' ');
        out.push_str("language-");
        out.push_str(&escape_attr(lang));
    }
    out.push_str("\"><code>");
    out.push_str(&generator.finalize());
    out.push_str("</code></pre>\n");
    Some(out)
}

/// Backup renderer in case syntect fails for whatever reason
pub fn fallback_plain(source: &str, language: Option<&str>) -> String {
    let mut out = String::with_capacity(source.len() + 32);
    out.push_str("<pre class=\"code\"><code");
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
