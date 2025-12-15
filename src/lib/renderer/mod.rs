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

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::test_support::{
        DEFAULT_CASES, LARGE_CASES, assert_ampersands_are_only_known_entities, counts,
        gen_any_utf8, gen_any_utf8_large, gen_events_general_finite,
        gen_events_well_formed_codeblock, gen_safe_html_text, gen_special_dense, ref_escape_html,
    };
    use proptest::collection;
    use proptest::prelude::*;
    use pulldown_cmark::{CowStr, Event, Tag, TagEnd, html};
    use std::sync::{Arc, Mutex};

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

    #[derive(Clone, Debug)]
    struct RecordingHighlighter {
        calls: Arc<Mutex<Vec<(String, Option<String>)>>>,
    }

    impl RecordingHighlighter {
        fn new(calls: Arc<Mutex<Vec<(String, Option<String>)>>>) -> Self {
            Self { calls }
        }
    }

    impl CodeblockHighlighter for RecordingHighlighter {
        fn render_codeblock(&self, source: &str, language: Option<&str>) -> Html {
            let rendered = format!(
                "<X-CODE lang={:?}>{}</X-CODE>",
                language,
                escape_html(source)
            );
            self.calls
                .lock()
                .expect("call log poisoned")
                .push((source.to_owned(), language.map(str::to_owned)));
            Html::from(rendered)
        }
    }

    #[derive(Clone, Debug)]
    struct RecordingMathRenderer {
        calls: Arc<Mutex<Vec<(String, bool)>>>,
    }

    impl RecordingMathRenderer {
        fn new(calls: Arc<Mutex<Vec<(String, bool)>>>) -> Self {
            Self { calls }
        }
    }

    impl MathRenderer for RecordingMathRenderer {
        fn render_math(&self, source: &str, display_mode: bool) -> Html {
            let rendered = format!(
                "<X-MATH display={}>{}</X-MATH>",
                display_mode,
                escape_html(source)
            );
            self.calls
                .lock()
                .expect("call log poisoned")
                .push((source.to_owned(), display_mode));
            Html::from(rendered)
        }
    }

    fn normalise_codeblock_sources(events: &[Event<'static>]) -> Vec<String> {
        let mut collected = Vec::new();
        let mut in_block = false;
        let mut current = String::new();
        for event in events {
            match event {
                Event::Start(Tag::CodeBlock(_)) => {
                    in_block = true;
                    current.clear();
                }
                Event::End(TagEnd::CodeBlock) if in_block => {
                    collected.push(current.clone());
                    in_block = false;
                }
                Event::Text(text) | Event::Code(text) if in_block => {
                    current.push_str(text.as_ref())
                }
                Event::SoftBreak | Event::HardBreak if in_block => current.push('\n'),
                Event::Html(html) | Event::InlineHtml(html) if in_block => {
                    current.push_str(html.as_ref())
                }
                Event::InlineMath(m) | Event::DisplayMath(m) if in_block => {
                    current.push_str(m.as_ref())
                }
                _ => {}
            }
        }
        collected
    }

    fn gen_multiple_codeblocks() -> impl Strategy<Value = Vec<Event<'static>>> {
        collection::vec(gen_events_well_formed_codeblock(), 0..=4)
            .prop_map(|blocks| blocks.into_iter().flatten().collect())
    }

    fn collect_math_calls(events: &[Event<'static>]) -> Vec<(String, bool)> {
        let mut calls = Vec::new();
        for event in events {
            match event {
                Event::InlineMath(m) => calls.push((m.to_string(), false)),
                Event::DisplayMath(m) => calls.push((m.to_string(), true)),
                _ => {}
            }
        }
        calls
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

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn codeblock_aggregation_matches_rule(events in gen_events_well_formed_codeblock()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = Renderer::new(
                RecordingHighlighter::new(code_calls.clone()),
                RecordingMathRenderer::new(math_calls),
            );

            let output = renderer.render(events.clone());
            let expected_sources = normalise_codeblock_sources(&events);
            let recorded = code_calls.lock().expect("call log poisoned");
            prop_assert_eq!(recorded.len(), expected_sources.len());
            for (idx, expected) in expected_sources.iter().enumerate() {
                let (ref recorded_source, _) = recorded[idx];
                prop_assert_eq!(recorded_source, expected);
            }
            let count_in_output = output.to_string().matches("<X-CODE").count();
            prop_assert_eq!(count_in_output, expected_sources.len());
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn exactly_one_replacement_per_code_block(events in gen_multiple_codeblocks()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = Renderer::new(
                RecordingHighlighter::new(code_calls.clone()),
                RecordingMathRenderer::new(Arc::new(Mutex::new(Vec::new()))),
            );

            let output = renderer.render(events.clone());
            let expected_sources = normalise_codeblock_sources(&events);
            let recorded = code_calls.lock().expect("call log poisoned");
            prop_assert_eq!(recorded.len(), expected_sources.len());
            let count_in_output = output.to_string().matches("<X-CODE").count();
            prop_assert_eq!(count_in_output, expected_sources.len());
        }
    }

    fn gen_math_sequences() -> impl Strategy<Value = Vec<Event<'static>>> {
        collection::vec(
            prop_oneof![
                gen_any_utf8().prop_map(|s| Event::InlineMath(CowStr::from(s))),
                gen_any_utf8().prop_map(|s| Event::DisplayMath(CowStr::from(s))),
                gen_any_utf8().prop_map(|s| Event::Text(CowStr::from(s))),
                Just(Event::SoftBreak),
                Just(Event::HardBreak),
            ],
            0..=16,
        )
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn math_dispatch_is_exact(events in gen_math_sequences()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = Renderer::new(
                RecordingHighlighter::new(code_calls),
                RecordingMathRenderer::new(math_calls.clone()),
            );

            let expected_maths = collect_math_calls(&events);
            let output = renderer.render(events.clone());

            let recorded = math_calls.lock().expect("math log poisoned");
            prop_assert_eq!(&*recorded, &expected_maths);

            let inline_markers = output.to_string().matches("<X-MATH display=false>").count();
            let display_markers = output.to_string().matches("<X-MATH display=true>").count();
            let expected_inline = expected_maths.iter().filter(|(_, display)| !*display).count();
            let expected_display = expected_maths.iter().filter(|(_, display)| *display).count();
            prop_assert_eq!(inline_markers, expected_inline);
            prop_assert_eq!(display_markers, expected_display);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn non_special_events_passthrough(events in gen_events_general_finite()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = Renderer::new(
                RecordingHighlighter::new(code_calls),
                RecordingMathRenderer::new(math_calls),
            );
            let mut expected = String::new();
            html::push_html(&mut expected, events.clone().into_iter());
            let rendered = renderer.render(events);
            prop_assert_eq!(rendered.to_string(), expected);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn renderer_completes_on_finite_input(events in prop_oneof![gen_events_general_finite(), gen_events_well_formed_codeblock()]) {
            let renderer = Renderer::new(
                RecordingHighlighter::new(Arc::new(Mutex::new(Vec::new()))),
                RecordingMathRenderer::new(Arc::new(Mutex::new(Vec::new()))),
            );
            let _ = renderer.render(events);
        }
    }
}
