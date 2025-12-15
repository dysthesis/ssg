use pulldown_cmark::Event;

trait CodeblockHighlighter {}

/// Renderer for parsed Markdown events to HTML
pub struct Renderer<'a, E, H>
where
    E: Iterator<Item = Event<'a>>,
    H: CodeblockHighlighter,
{
    /// The iterator over parsed Events from pulldown_cmark
    parser: E,
    /// How to highlight code blocks
    code_block_highlighter: H,
}
