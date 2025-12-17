pub mod katex;

use crate::document::Html;

pub trait MathRenderer {
    fn render_math(&self, source: &str, display_mode: bool) -> Html;
}

// Re-export for convenience
pub use crate::highlighter::escape_html;
pub use katex::KATEX_STYLESHEET_LINK;
