pub mod katex;
pub mod syntect;

use pulldown_cmark::{CowStr, Event, Tag, TagEnd, html};
use tracing::warn;

use crate::document::Html;

pub trait CodeblockHighlighter {
    fn render_codeblock(&self, source: &str, language: Option<&str>) -> Html;
}

pub trait MathRenderer {
    fn render_math(&self, source: &str, display_mode: bool) -> Html;
}

/// Renderer for parsed Markdown events to HTML
pub struct Renderer<H, M>
where
    H: CodeblockHighlighter,
    M: MathRenderer,
{
    /// How to highlight code blocks
    code_block_highlighter: H,
    /// How to render LaTeX fragments
    math_renderer: M,
}

impl<H, M> Renderer<H, M>
where
    H: CodeblockHighlighter,
    M: MathRenderer,
{
    pub fn new(code_block_highlighter: H, math_renderer: M) -> Self {
        Self {
            code_block_highlighter,
            math_renderer,
        }
    }

    /// Consume the event iterator and render it to an HTML body
    pub fn render<'a, E>(&self, events: E) -> Html
    where
        E: IntoIterator<Item = Event<'a>>,
    {
        let mut parser = events.into_iter().peekable();

        let mut translated_events: Vec<Event<'a>> = Vec::new();

        while let Some(event) = parser.next() {
            match event {
                Event::Start(Tag::CodeBlock(kind)) => {
                    let language: Option<String> = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(lang) => Some(lang.into_string()),
                        _ => None,
                    };
                    let mut code = String::new();

                    for inner in parser.by_ref() {
                        match inner {
                            Event::End(TagEnd::CodeBlock) => break,
                            Event::Text(text) | Event::Code(text) => code.push_str(text.as_ref()),
                            Event::SoftBreak | Event::HardBreak => code.push('\n'),
                            Event::Html(html) | Event::InlineHtml(html) => {
                                code.push_str(html.as_ref())
                            }
                            Event::InlineMath(math) | Event::DisplayMath(math) => {
                                code.push_str(math.as_ref())
                            }
                            other => warn!("Unexpected event inside code block: {other:?}"),
                        }
                    }

                    let highlighted = self
                        .code_block_highlighter
                        .render_codeblock(&code, language.as_deref())
                        .to_string();
                    translated_events.push(Event::Html(CowStr::from(highlighted)));
                }
                Event::InlineMath(source) => {
                    let rendered = self
                        .math_renderer
                        .render_math(source.as_ref(), false)
                        .to_string();
                    translated_events.push(Event::InlineHtml(CowStr::from(rendered)));
                }
                Event::DisplayMath(source) => {
                    let rendered = self
                        .math_renderer
                        .render_math(source.as_ref(), true)
                        .to_string();
                    translated_events.push(Event::Html(CowStr::from(rendered)));
                }
                other => translated_events.push(other),
            }
        }

        let mut output = String::new();
        html::push_html(&mut output, translated_events.into_iter());

        Html::from(output)
    }
}

impl Default for Renderer<syntect::SyntectHighlighter, katex::KatexRenderer> {
    fn default() -> Self {
        Self::new(
            syntect::SyntectHighlighter::default(),
            katex::KatexRenderer::new(),
        )
    }
}

pub(super) fn escape_html(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
