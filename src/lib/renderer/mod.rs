pub mod jobs;
pub mod katex;
pub mod syntect;

use color_eyre::eyre::Result;
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
    H: CodeblockHighlighter + Send + Clone + Sync + 'static,
    M: MathRenderer + Send + Clone + Sync + 'static,
{
    /// How to highlight code blocks
    code_block_highlighter: H,
    /// How to render LaTeX fragments
    math_renderer: M,
}

impl<H, M> Renderer<H, M>
where
    H: CodeblockHighlighter + Send + Clone + Sync + 'static,
    M: MathRenderer + Send + Clone + Sync + 'static,
{
    /// Construct a new instance of a renderer
    pub fn new(code_block_highlighter: H, math_renderer: M) -> Self {
        Self {
            code_block_highlighter,
            math_renderer,
        }
    }

    /// Consume the event iterator and render it to an HTML body
    pub fn render<'a, E>(&self, events: E) -> Result<Html>
    where
        E: IntoIterator<Item = Event<'a>>,
    {
        // Single-pass collection with job detection
        let mut events_vec: Vec<Event<'a>> = Vec::new();
        let mut has_jobs = false;

        for event in events {
            if matches!(
                event,
                Event::Start(Tag::CodeBlock(_)) | Event::InlineMath(_) | Event::DisplayMath(_)
            ) {
                has_jobs = true;
            }
            events_vec.push(event);
        }

        // Fast path: no jobs, render directly
        if !has_jobs {
            let mut output = String::new();
            html::push_html(&mut output, events_vec.into_iter());
            return Ok(Html::from(output));
        }

        // Slow path: existing job-based rendering
        let mut parser = events_vec.into_iter().peekable();
        let mut translated_events: Vec<Event<'static>> = Vec::new();

        // Storage for job source data - these need to live until jobs are executed
        let mut code_sources: Vec<String> = Vec::new();
        let mut code_languages: Vec<String> = Vec::new();
        let mut inline_math_sources: Vec<String> = Vec::new();
        let mut display_math_sources: Vec<String> = Vec::new();

        // Track job specifications (event index, data index)
        let mut code_job_specs: Vec<(usize, usize, Option<usize>)> = Vec::new();
        let mut inline_math_specs: Vec<(usize, usize)> = Vec::new();
        let mut display_math_specs: Vec<(usize, usize)> = Vec::new();

        // First pass: translate events and collect jobs
        while let Some(event) = parser.next() {
            match event {
                // Parse a code block
                Event::Start(Tag::CodeBlock(kind)) => {
                    let language: Option<String> = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(lang) => Some(lang.into_string()),
                        // TODO: Figure out what best to do with indented code
                        // blocks
                        pulldown_cmark::CodeBlockKind::Indented => None,
                    };

                    let mut code = String::new();

                    // Parse the inner content of code blocks
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

                    // Store job data and create placeholder
                    let source_idx = code_sources.len();
                    code_sources.push(code);

                    let lang_idx = if let Some(lang) = language {
                        let idx = code_languages.len();
                        code_languages.push(lang);
                        Some(idx)
                    } else {
                        None
                    };

                    let event_idx = translated_events.len();
                    code_job_specs.push((event_idx, source_idx, lang_idx));
                    translated_events.push(Event::Html(CowStr::from(""))); // placeholder
                }
                // Collect inline math jobs
                Event::InlineMath(source) => {
                    let source_idx = inline_math_sources.len();
                    inline_math_sources.push(source.into_string());

                    let event_idx = translated_events.len();
                    inline_math_specs.push((event_idx, source_idx));
                    translated_events.push(Event::InlineHtml(CowStr::from(""))); // placeholder
                }
                // Collect display math jobs
                Event::DisplayMath(source) => {
                    let source_idx = display_math_sources.len();
                    display_math_sources.push(source.into_string());

                    let event_idx = translated_events.len();
                    display_math_specs.push((event_idx, source_idx));
                    translated_events.push(Event::Html(CowStr::from(""))); // placeholder
                }

                // Convert other events to owned/static
                other => translated_events.push(to_static_event(other)),
            }
        }

        // Second pass: create jobs and execute them
        let total_jobs = code_job_specs.len() + inline_math_specs.len() + display_math_specs.len();

        if total_jobs > 0 {
            // Create code block jobs
            let code_jobs: Vec<_> = code_job_specs
                .iter()
                .map(|(event_idx, source_idx, lang_idx)| jobs::CodeBlockJob {
                    highlighter: self.code_block_highlighter.clone(),
                    idx: *event_idx,
                    source: code_sources[*source_idx].clone(),
                    lang: lang_idx
                        .map(|li| code_languages[li].clone())
                        .unwrap_or_default(),
                })
                .collect();

            // Create inline math jobs
            let inline_math_jobs: Vec<_> = inline_math_specs
                .iter()
                .map(|(event_idx, source_idx)| jobs::InlineMathJob {
                    renderer: self.math_renderer.clone(),
                    idx: *event_idx,
                    source: inline_math_sources[*source_idx].clone(),
                })
                .collect();

            // Create display math jobs
            let display_math_jobs: Vec<_> = display_math_specs
                .iter()
                .map(|(event_idx, source_idx)| jobs::DisplayMathJob {
                    renderer: self.math_renderer.clone(),
                    idx: *event_idx,
                    source: display_math_sources[*source_idx].clone(),
                })
                .collect();

            // Collect all jobs as trait objects
            let mut job_refs: Vec<&dyn jobs::Job> = Vec::new();
            for job in &code_jobs {
                job_refs.push(job);
            }
            for job in &inline_math_jobs {
                job_refs.push(job);
            }
            for job in &display_math_jobs {
                job_refs.push(job);
            }

            // Execute jobs
            let jobs_queue: jobs::Jobs = job_refs.into_iter().collect();
            let results = jobs_queue.execute()?;

            // Third pass: replace placeholders with results
            for (idx, event) in results {
                translated_events[idx] = event;
            }
        }

        let mut output = String::new();

        // Leave it to pulldown_cmark to translate everything else
        html::push_html(&mut output, translated_events.into_iter());
        Ok(Html::from(output))
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

/// Convert an event with any lifetime to an owned event with static lifetime
fn to_static_event(event: Event<'_>) -> Event<'static> {
    // Most pulldown_cmark events with Tag/TagEnd don't actually have lifetime-dependent data
    // Tags like Paragraph, Heading, etc. are already 'static
    // The few that have CowStr fields (like Link, Image) need special handling
    // For now, we use unsafe transmute since we know the Tag data is either 'static or cloned
    match event {
        Event::Start(tag) => {
            Event::Start(unsafe { std::mem::transmute::<Tag<'_>, Tag<'static>>(tag) })
        }
        Event::End(tag) => Event::End(tag),
        Event::Text(s) => Event::Text(CowStr::from(s.into_string())),
        Event::Code(s) => Event::Code(CowStr::from(s.into_string())),
        Event::Html(s) => Event::Html(CowStr::from(s.into_string())),
        Event::InlineHtml(s) => Event::InlineHtml(CowStr::from(s.into_string())),
        Event::FootnoteReference(s) => Event::FootnoteReference(CowStr::from(s.into_string())),
        Event::SoftBreak => Event::SoftBreak,
        Event::HardBreak => Event::HardBreak,
        Event::Rule => Event::Rule,
        Event::TaskListMarker(checked) => Event::TaskListMarker(checked),
        Event::InlineMath(s) => Event::InlineMath(CowStr::from(s.into_string())),
        Event::DisplayMath(s) => Event::DisplayMath(CowStr::from(s.into_string())),
    }
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

    type CodeCallLog = Arc<Mutex<Vec<(String, Option<String>)>>>;

    #[derive(Clone, Debug)]
    struct RecordingHighlighter {
        calls: CodeCallLog,
    }

    impl RecordingHighlighter {
        fn new(calls: CodeCallLog) -> Self {
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

            let output = renderer.render(events.clone()).expect("render should succeed");
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

            let output = renderer.render(events.clone()).expect("render should succeed");
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
            let output = renderer.render(events.clone()).expect("render should succeed");

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
            let rendered = renderer.render(events).expect("render should succeed");
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
