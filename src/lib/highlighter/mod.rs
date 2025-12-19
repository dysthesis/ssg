pub mod syntect;

use crate::document::Html;

pub trait CodeblockHighlighter {
    fn render_codeblock(&self, source: &str, language: Option<&str>) -> Html;
}

pub fn escape_html(raw: &str) -> String {
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

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::test_support::{
        DEFAULT_CASES, LARGE_CASES, assert_ampersands_are_only_known_entities, counts,
        gen_any_utf8, gen_any_utf8_large, gen_safe_html_text, gen_special_dense, ref_escape_html,
    };
    use proptest::prelude::*;

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: DEFAULT_CASES,
            ..ProptestConfig::default()
        }
    }

    fn large_config() -> ProptestConfig {
        ProptestConfig {
            cases: LARGE_CASES,
            ..ProptestConfig::default()
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn escape_html_matches_reference(s in gen_any_utf8()) {
            let escaped = escape_html(&s);
            prop_assert_eq!(escaped, ref_escape_html(&s));
        }
    }

    proptest! {
        #![proptest_config(large_config())]
        #[test]
        fn escape_html_large_inputs(s in gen_any_utf8_large()) {
            let escaped = escape_html(&s);
            prop_assert_eq!(escaped, ref_escape_html(&s));
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn escape_html_removes_raw_brackets_and_quotes(s in gen_any_utf8()) {
            let escaped = escape_html(&s);
            prop_assert!(!escaped.contains('<'));
            prop_assert!(!escaped.contains('>'));
            prop_assert!(!escaped.contains('"'));
            prop_assert!(!escaped.contains('\''));
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn escape_html_ampersands_form_known_entities(s in gen_any_utf8()) {
            let escaped = escape_html(&s);
            assert_ampersands_are_only_known_entities(&escaped)?;
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn escape_html_length_identity(s in gen_special_dense()) {
            let escaped = escape_html(&s);
            let (n_amp, n_lt, n_gt, n_dq, n_sq) = counts(&s);
            let expected = s.len() + (4 * n_amp) + (3 * n_lt) + (3 * n_gt) + (5 * n_dq) + (5 * n_sq);
            prop_assert_eq!(escaped.len(), expected);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn escape_html_identity_on_safe_input(s in gen_safe_html_text()) {
            let escaped = escape_html(&s);
            prop_assert_eq!(escaped, s);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn escape_html_concatenation_homomorphism(a in gen_any_utf8(), b in gen_any_utf8()) {
            let combined = format!("{a}{b}");
            let escaped_combined = escape_html(&combined);
            let separated = format!("{}{}", escape_html(&a), escape_html(&b));
            prop_assert_eq!(escaped_combined, separated);
        }
    }
}
