use katex::Opts;
use tracing::warn;

use crate::{
    document::Html,
    math::{MathRenderer, escape_html},
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

pub fn fallback_plain_math(source: &str, display_mode: bool) -> String {
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

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::test_support::{DEFAULT_CASES, gen_any_utf8, gen_display_mode};
    use proptest::prelude::*;

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: DEFAULT_CASES,
            ..ProptestConfig::default()
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn fallback_plain_math_wraps_correctly(source in gen_any_utf8(), display_mode in gen_display_mode()) {
            let rendered = fallback_plain_math(&source, display_mode);
            let (prefix, suffix) = if display_mode {
                ("<div class=\"math math-display\">", "</div>")
            } else {
                ("<span class=\"math math-inline\">", "</span>")
            };

            prop_assert!(rendered.starts_with(prefix));
            prop_assert!(rendered.ends_with(suffix));

            let middle = &rendered[prefix.len()..rendered.len() - suffix.len()];
            let escaped = escape_html(&source);
            prop_assert_eq!(middle, escaped);
            prop_assert!(!middle.contains('<'));
            prop_assert!(!middle.contains('>'));
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn render_math_matches_katex_or_fallback(source in gen_any_utf8(), display_mode in gen_display_mode()) {
            let renderer = KatexRenderer::new();
            let produced = renderer.render_math(&source, display_mode).to_string();

            let mut builder = Opts::builder();
            builder.display_mode(display_mode);
            let opts = builder.build().unwrap_or_else(|_| Opts::default());

            match katex::render_with_opts(&source, &opts) {
                Ok(html) => prop_assert_eq!(produced, html),
                Err(_) => prop_assert_eq!(produced, fallback_plain_math(&source, display_mode)),
            }
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn render_math_is_deterministic(source in gen_any_utf8(), display_mode in gen_display_mode()) {
            let renderer = KatexRenderer::new();
            let first = renderer.render_math(&source, display_mode).to_string();
            let second = renderer.render_math(&source, display_mode).to_string();
            prop_assert_eq!(first, second);
        }
    }
}
