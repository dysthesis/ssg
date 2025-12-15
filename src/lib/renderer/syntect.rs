use std::sync::OnceLock;

use syntect::{
    highlighting::ThemeSet,
    html::highlighted_html_for_string,
    parsing::{SyntaxReference, SyntaxSet},
};
use tracing::warn;

use crate::{
    document::Html,
    renderer::{CodeblockHighlighter, escape_html},
};

#[derive(Default)]
pub struct SyntectHighlighter {}
impl CodeblockHighlighter for SyntectHighlighter {
    fn render_codeblock(&self, source: &str, language: Option<&str>) -> Html {
        let syntax_set = syntax_set();
        let theme = theme();

        let syntax: &SyntaxReference = language
            .and_then(|lang| syntax_set.find_syntax_by_token(lang))
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

        match highlighted_html_for_string(source, syntax_set, syntax, theme) {
            Ok(rendered) => Html::from(rendered),
            Err(error) => {
                warn!(?language, %error, "Falling back to plain code block rendering");
                Html::from(fallback_plain(source, language))
            }
        }
    }
}

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn theme() -> &'static syntect::highlighting::Theme {
    let themes = &theme_set().themes;
    themes
        .get("InspiredGitHub")
        .or_else(|| themes.values().next())
        .expect("syntect default themes should not be empty")
}

fn fallback_plain(source: &str, language: Option<&str>) -> String {
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
