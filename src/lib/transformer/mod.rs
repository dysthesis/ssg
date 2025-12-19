use pulldown_cmark::Event;

pub mod code_block;
pub mod math;

/// A transformer layer over an iterator of events, in order to allow custom
/// rendering strategies of different syntax elements
pub trait Transformer<'a>: Iterator<Item = Event<'a>> {}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::{
        document::Html,
        highlighter::{escape_html, CodeblockHighlighter},
        math::MathRenderer,
        test_support::{gen_any_utf8, gen_events_well_formed_codeblock, DEFAULT_CASES},
    };
    use code_block::ToCodeBlockTransformer;
    use math::ToMathTransformer;
    use proptest::{collection, prelude::*};
    use pulldown_cmark::{html, CowStr, Event, Tag};
    use std::sync::{Arc, Mutex};

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: DEFAULT_CASES,
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

    fn gen_mixed_events() -> impl Strategy<Value = Vec<Event<'static>>> {
        collection::vec(
            prop_oneof![
                gen_any_utf8().prop_map(|s| Event::Text(CowStr::from(s))),
                gen_any_utf8().prop_map(|s| Event::InlineMath(CowStr::from(s))),
                gen_any_utf8().prop_map(|s| Event::DisplayMath(CowStr::from(s))),
                Just(Event::SoftBreak),
                Just(Event::HardBreak),
            ],
            0..=16,
        )
    }

    fn gen_mixed_with_codeblocks() -> impl Strategy<Value = Vec<Event<'static>>> {
        (
            gen_events_well_formed_codeblock(),
            gen_mixed_events(),
            gen_events_well_formed_codeblock(),
        )
            .prop_map(|(mut code1, mut middle, mut code2)| {
                let mut events = Vec::new();
                events.append(&mut code1);
                events.append(&mut middle);
                events.append(&mut code2);
                events
            })
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn composed_transformers_handle_both_code_and_math(events in gen_mixed_with_codeblocks()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls.clone());
            let math_renderer = RecordingMathRenderer::new(math_calls.clone());

            let code_count = events.iter().filter(|e| matches!(e, Event::Start(Tag::CodeBlock(_)))).count();
            let inline_math_count = events.iter().filter(|e| matches!(e, Event::InlineMath(_))).count();
            let display_math_count = events.iter().filter(|e| matches!(e, Event::DisplayMath(_))).count();

            let _transformed: Vec<Event> = events
                .into_iter()
                .highlight_code(&highlighter)
                .render_math(&math_renderer)
                .collect();

            let code_recorded = code_calls.lock().expect("code log poisoned");
            let math_recorded = math_calls.lock().expect("math log poisoned");

            prop_assert_eq!(code_recorded.len(), code_count);
            prop_assert_eq!(math_recorded.len(), inline_math_count + display_math_count);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn transformer_order_independence_for_disjoint_events(
            code_events in gen_events_well_formed_codeblock(),
            math_events in gen_mixed_events()
        ) {
            let code_calls1 = Arc::new(Mutex::new(Vec::new()));
            let math_calls1 = Arc::new(Mutex::new(Vec::new()));
            let code_calls2 = Arc::new(Mutex::new(Vec::new()));
            let math_calls2 = Arc::new(Mutex::new(Vec::new()));

            let highlighter1 = RecordingHighlighter::new(code_calls1.clone());
            let math_renderer1 = RecordingMathRenderer::new(math_calls1.clone());
            let highlighter2 = RecordingHighlighter::new(code_calls2.clone());
            let math_renderer2 = RecordingMathRenderer::new(math_calls2.clone());

            let mut events1 = code_events.clone();
            events1.extend(math_events.clone());
            let events2 = events1.clone();

            let output1: Vec<Event> = events1
                .into_iter()
                .highlight_code(&highlighter1)
                .render_math(&math_renderer1)
                .collect();

            let output2: Vec<Event> = events2
                .into_iter()
                .render_math(&math_renderer2)
                .highlight_code(&highlighter2)
                .collect();

            let mut html1 = String::new();
            html::push_html(&mut html1, output1.into_iter());

            let mut html2 = String::new();
            html::push_html(&mut html2, output2.into_iter());

            let code1 = code_calls1.lock().unwrap();
            let code2 = code_calls2.lock().unwrap();
            let math1 = math_calls1.lock().unwrap();
            let math2 = math_calls2.lock().unwrap();

            prop_assert_eq!(code1.len(), code2.len());
            prop_assert_eq!(math1.len(), math2.len());
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn chained_transformers_complete_on_finite_input(events in gen_mixed_with_codeblocks()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls);
            let math_renderer = RecordingMathRenderer::new(math_calls);

            let _transformed: Vec<Event> = events
                .into_iter()
                .highlight_code(&highlighter)
                .render_math(&math_renderer)
                .collect();
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn empty_input_produces_empty_output(
            _unit in Just(())
        ) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls.clone());
            let math_renderer = RecordingMathRenderer::new(math_calls.clone());

            let events: Vec<Event> = Vec::new();
            let transformed: Vec<Event> = events
                .into_iter()
                .highlight_code(&highlighter)
                .render_math(&math_renderer)
                .collect();

            prop_assert_eq!(transformed.len(), 0);
            prop_assert_eq!(code_calls.lock().unwrap().len(), 0);
            prop_assert_eq!(math_calls.lock().unwrap().len(), 0);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn single_text_event_passes_through(text in gen_any_utf8()) {
            let code_calls = Arc::new(Mutex::new(Vec::new()));
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let highlighter = RecordingHighlighter::new(code_calls.clone());
            let math_renderer = RecordingMathRenderer::new(math_calls.clone());

            let events = vec![Event::Text(CowStr::from(text.clone()))];
            let transformed: Vec<Event> = events
                .into_iter()
                .highlight_code(&highlighter)
                .render_math(&math_renderer)
                .collect();

            prop_assert_eq!(transformed.len(), 1);
            if let Event::Text(t) = &transformed[0] {
                prop_assert_eq!(t.as_ref(), text.as_str());
            } else {
                prop_assert!(false, "Expected Text event");
            }

            prop_assert_eq!(code_calls.lock().unwrap().len(), 0);
            prop_assert_eq!(math_calls.lock().unwrap().len(), 0);
        }
    }
}
