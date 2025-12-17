use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};

use crate::highlighter::CodeblockHighlighter;
use crate::transformer::Transformer;

pub enum CodeBlockState<'a> {
    /// Not in code block, pass through the event as-is.
    Passthrough,
    /// Currently inside a code block of language `lang`, so we accumulate all
    /// events until an `Event::End(TagEnd::CodeBlock)` is reached.
    Accumulating { lang: CodeBlockKind<'a> },
}

/// An adapter over pulldown_cmark parser in order to render code blocks with
/// custom strategies, e.g. tree-sitter-based highlighting using `syntect`
pub struct CodeBlockTransformer<'a, I, H>
where
    I: Iterator<Item = Event<'a>>,
    H: CodeblockHighlighter,
{
    /// The inner iterator. Can be the raw `Parser`, another `Transformer`, or
    /// other iterators over `Event<'a>`.
    inner: I,
    /// The highlighting strategy to use.
    highlighter: &'a H,
    /// Buffer to accumulate any code.
    buffer: String,
    /// Current state of the transformer; are we inside a code block?
    state: CodeBlockState<'a>, // events are bound to the Markdown source
                               // string; likewise is the language name for the
                               // current state
}

impl<'a, I, H> Iterator for CodeBlockTransformer<'a, I, H>
where
    I: Iterator<Item = Event<'a>>,
    H: CodeblockHighlighter,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let event = self.inner.next()?;
            match &self.state {
                CodeBlockState::Passthrough => match event {
                    Event::Start(Tag::CodeBlock(lang)) => {
                        // Transition to accumulating state
                        self.state = CodeBlockState::Accumulating { lang };
                        self.buffer.clear();
                        // Don't return anything—swallow the Start event
                        continue;
                    }
                    // All other events pass through unchanged
                    other => return Some(other),
                },
                CodeBlockState::Accumulating { lang: _ } => {
                    match event {
                        Event::End(TagEnd::CodeBlock) => {
                            // Extract the kind before transitioning state
                            let CodeBlockState::Accumulating { lang } =
                                std::mem::replace(&mut self.state, CodeBlockState::Passthrough)
                            else {
                                unreachable!()
                            };

                            // Convert CodeBlockKind to Option<&str>
                            let language = match lang {
                                CodeBlockKind::Fenced(ref l) => Some(l.as_ref()),
                                CodeBlockKind::Indented => None,
                            };

                            // Perform the actual highlighting
                            let html = self.highlighter.render_codeblock(&self.buffer, language);

                            // Return the transformed content as an Html event
                            return Some(Event::Html(html.into_cow_str()));
                        }
                        Event::Text(text) | Event::Code(text) => {
                            self.buffer.push_str(text.as_ref());
                            continue;
                        }
                        Event::SoftBreak | Event::HardBreak => {
                            self.buffer.push('\n');
                            continue;
                        }
                        Event::Html(html) | Event::InlineHtml(html) => {
                            self.buffer.push_str(html.as_ref());
                            continue;
                        }
                        Event::InlineMath(math) | Event::DisplayMath(math) => {
                            self.buffer.push_str(math.as_ref());
                            continue;
                        }
                        other => {
                            tracing::warn!("Unexpected event inside code block: {:?}", other);
                            continue;
                        }
                    }
                }
            }
        }
    }
}

impl<'a, I, H> Transformer<'a> for CodeBlockTransformer<'a, I, H>
where
    I: Iterator<Item = Event<'a>>,
    H: CodeblockHighlighter,
{
}

pub trait ToCodeBlockTransformer<'a>: Iterator<Item = Event<'a>> + Sized {
    fn highlight_code<H>(self, highlighter: &'a H) -> CodeBlockTransformer<'a, Self, H>
    where
        H: CodeblockHighlighter,
    {
        CodeBlockTransformer {
            inner: self,
            buffer: String::new(),
            state: CodeBlockState::Passthrough,
            highlighter,
        }
    }
}

impl<'a, I> ToCodeBlockTransformer<'a> for I where I: Iterator<Item = Event<'a>> {}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::{
        document::Html,
        highlighter::{escape_html, CodeblockHighlighter},
        test_support::{
            gen_any_utf8, gen_events_well_formed_codeblock, gen_language_token_adversarial,
            DEFAULT_CASES,
        },
    };
    use proptest::{collection, option, prelude::*};
    use pulldown_cmark::{html, CowStr, Event, Tag, TagEnd};
    use std::sync::{Arc, Mutex};

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: DEFAULT_CASES,
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
                Event::Text(text) if in_block => current.push_str(text.as_ref()),
                Event::Code(text) if in_block => current.push_str(text.as_ref()),
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

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn codeblock_aggregation_matches_rule(events in gen_events_well_formed_codeblock()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls.clone());

            let transformed: Vec<Event> = events
                .clone()
                .into_iter()
                .highlight_code(&highlighter)
                .collect();

            let expected_sources = normalise_codeblock_sources(&events);
            let recorded = code_calls.lock().expect("call log poisoned");
            prop_assert_eq!(recorded.len(), expected_sources.len());

            for (idx, expected) in expected_sources.iter().enumerate() {
                let (ref recorded_source, _) = recorded[idx];
                prop_assert_eq!(recorded_source, expected);
            }

            let mut output = String::new();
            html::push_html(&mut output, transformed.into_iter());
            let count_in_output = output.matches("<X-CODE").count();
            prop_assert_eq!(count_in_output, expected_sources.len());
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn exactly_one_replacement_per_code_block(events in gen_multiple_codeblocks()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls.clone());

            let transformed: Vec<Event> = events
                .clone()
                .into_iter()
                .highlight_code(&highlighter)
                .collect();

            let expected_sources = normalise_codeblock_sources(&events);
            let recorded = code_calls.lock().expect("call log poisoned");
            prop_assert_eq!(recorded.len(), expected_sources.len());

            let mut output = String::new();
            html::push_html(&mut output, transformed.into_iter());
            let count_in_output = output.matches("<X-CODE").count();
            prop_assert_eq!(count_in_output, expected_sources.len());
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn transformer_preserves_non_codeblock_events(
            before in collection::vec(gen_any_utf8().prop_map(|s| Event::Text(CowStr::from(s))), 0..=4),
            code_events in gen_events_well_formed_codeblock(),
            after in collection::vec(gen_any_utf8().prop_map(|s| Event::Text(CowStr::from(s))), 0..=4)
        ) {
            let mut events = Vec::new();
            events.extend(before.clone());
            events.extend(code_events);
            events.extend(after.clone());

            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls);

            let transformed: Vec<Event> = events
                .into_iter()
                .highlight_code(&highlighter)
                .collect();

            let first_html_idx = transformed
                .iter()
                .position(|e| matches!(e, Event::Html(_)));

            if let Some(idx) = first_html_idx {
                for i in 0..idx {
                    if i < before.len() {
                        prop_assert!(matches!(transformed[i], Event::Text(_)));
                    }
                }
            }
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn transformer_completes_on_finite_input(events in prop_oneof![
            gen_events_well_formed_codeblock(),
            collection::vec(gen_any_utf8().prop_map(|s| Event::Text(CowStr::from(s))), 0..=16)
        ]) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls);
            let _transformed: Vec<Event> = events
                .into_iter()
                .highlight_code(&highlighter)
                .collect();
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn language_parameter_passed_correctly(
            source in gen_any_utf8(),
            language in option::of(gen_language_token_adversarial())
        ) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls.clone());

            let kind = match language.clone() {
                Some(lang) => CodeBlockKind::Fenced(CowStr::from(lang)),
                None => CodeBlockKind::Indented,
            };

            let events = vec![
                Event::Start(Tag::CodeBlock(kind)),
                Event::Text(CowStr::from(source.clone())),
                Event::End(TagEnd::CodeBlock),
            ];

            let _transformed: Vec<Event> = events
                .into_iter()
                .highlight_code(&highlighter)
                .collect();

            let recorded = code_calls.lock().expect("call log poisoned");
            prop_assert_eq!(recorded.len(), 1);
            let (recorded_source, recorded_lang) = &recorded[0];
            prop_assert_eq!(recorded_source, &source);
            prop_assert_eq!(recorded_lang.as_deref(), language.as_deref());
        }
    }
}
