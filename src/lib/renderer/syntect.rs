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

#[derive(Default, Clone)]
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

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::test_support::{
        DEFAULT_CASES, decode_five_entities, gen_any_utf8, gen_language_token_adversarial,
        strip_tags_lossy,
    };
    use proptest::option;
    use proptest::prelude::*;

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: DEFAULT_CASES,
            ..ProptestConfig::default()
        }
    }

    fn normalise_newlines_allow_single_trailing(s: &str) -> String {
        let mut norm = s.replace("\r\n", "\n");
        let had_trailing = norm.ends_with('\n');
        while norm.ends_with('\n') {
            norm.pop();
        }
        if had_trailing {
            norm.push('\n');
        }
        norm
    }

    fn recover_text(html: &str) -> String {
        let stripped = strip_tags_lossy(html);
        let mut decoded = decode_five_entities(&stripped);
        if decoded.starts_with('\n') {
            decoded.remove(0);
        }
        if decoded.ends_with('\n') {
            decoded.pop();
        }
        decoded
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn fallback_plain_structure_and_content(source in gen_any_utf8(), language in option::of(gen_language_token_adversarial())) {
            let rendered = fallback_plain(&source, language.as_deref());
            prop_assert!(rendered.starts_with("<pre><code"));
            prop_assert!(rendered.ends_with("</code></pre>\n"));

            let escaped = escape_html(&source);
            if escaped.is_empty() {
                let end = rendered.rfind("</code>").expect("code close");
                let start = rendered[..end].rfind('>').expect("code open end") + 1;
                prop_assert_eq!(&rendered[start..end], "");
            } else {
                prop_assert_eq!(rendered.match_indices(&escaped).count(), 1);
            }

            prop_assert!(rendered.ends_with('\n'));
            if rendered.len() > 1 {
                prop_assert!(!rendered[..rendered.len()-1].ends_with('\n'));
            }
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn render_codeblock_textual_recovery(source in gen_any_utf8(), language in option::of(gen_language_token_adversarial())) {
            let highlighter = SyntectHighlighter::default();
            let rendered = highlighter.render_codeblock(&source, language.as_deref()).to_string();
            let recovered = recover_text(&rendered);
            let expected = normalise_newlines_allow_single_trailing(&source);
            let recovered_norm = normalise_newlines_allow_single_trailing(&recovered);
            prop_assert_eq!(recovered_norm, expected);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn render_codeblock_language_independence(source in gen_any_utf8(), lang1 in option::of(gen_language_token_adversarial()), lang2 in option::of(gen_language_token_adversarial())) {
            let highlighter = SyntectHighlighter::default();
            let rendered1 = highlighter.render_codeblock(&source, lang1.as_deref()).to_string();
            let rendered2 = highlighter.render_codeblock(&source, lang2.as_deref()).to_string();
            let recovered1 = normalise_newlines_allow_single_trailing(&recover_text(&rendered1));
            let recovered2 = normalise_newlines_allow_single_trailing(&recover_text(&rendered2));
            prop_assert_eq!(recovered1, recovered2);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn render_codeblock_never_panics(source in gen_any_utf8(), language in option::of(gen_language_token_adversarial())) {
            let highlighter = SyntectHighlighter::default();
            let _ = highlighter.render_codeblock(&source, language.as_deref());
        }
    }
}
