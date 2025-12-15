use katex::Opts;
use tracing::warn;

use crate::{
    document::Html,
    renderer::{MathRenderer, escape_html},
};

/// Renders LaTeX to KaTeX HTML + MathML without requiring client-side JS.
#[derive(Default)]
pub struct KatexRenderer;

impl KatexRenderer {
    pub const fn new() -> Self {
        Self
    }
}

impl MathRenderer for KatexRenderer {
    fn render_math(&self, source: &str, display_mode: bool) -> Html {
        let mut builder = Opts::builder();
        builder.display_mode(display_mode);

        let opts = match builder.build() {
            Ok(opts) => opts,
            Err(error) => {
                warn!(%error, "Falling back to KaTeX default options");
                Opts::default()
            }
        };

        match katex::render_with_opts(source, &opts) {
            Ok(html) => Html::from(html),
            Err(error) => {
                warn!(%error, "Failed to render KaTeX, returning escaped source");
                Html::from(fallback_plain_math(source, display_mode))
            }
        }
    }
}

pub const KATEX_STYLESHEET_LINK: &str = r#"<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.4/dist/katex.min.css" crossorigin="anonymous">"#;

fn fallback_plain_math(source: &str, display_mode: bool) -> String {
    let mut out = String::with_capacity(source.len() + 32);
    if display_mode {
        out.push_str("<div class=\"math math-display\">");
    } else {
        out.push_str("<span class=\"math math-inline\">");
    }

    out.push_str(&escape_html(source));

    if display_mode {
        out.push_str("</div>");
    } else {
        out.push_str("</span>");
    }
    out
}
