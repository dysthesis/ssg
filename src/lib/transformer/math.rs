use pulldown_cmark::Event;

use crate::math::MathRenderer;
use crate::transformer::Transformer;

/// An adapter over pulldown_cmark parser in order to render math expressions
/// with custom strategies, e.g. KaTeX-based server-side rendering.
pub struct MathTransformer<'a, I, M>
where
    I: Iterator<Item = Event<'a>>,
    M: MathRenderer,
{
    /// The inner iterator. Can be the raw `Parser`, another `Transformer`, or
    /// other iterators over `Event<'a>`.
    inner: I,
    /// The rendering strategy to use.
    renderer: &'a M,
}

impl<'a, I, M> Iterator for MathTransformer<'a, I, M>
where
    I: Iterator<Item = Event<'a>>,
    M: MathRenderer,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Event::InlineMath(source) => {
                let html = self.renderer.render_math(source.as_ref(), false);
                Some(Event::InlineHtml(html.into_cow_str()))
            }
            Event::DisplayMath(source) => {
                let html = self.renderer.render_math(source.as_ref(), true);
                Some(Event::Html(html.into_cow_str()))
            }
            other => Some(other),
        }
    }
}

impl<'a, I, M> Transformer<'a> for MathTransformer<'a, I, M>
where
    I: Iterator<Item = Event<'a>>,
    M: MathRenderer,
{
}

pub trait ToMathTransformer<'a>: Iterator<Item = Event<'a>> + Sized {
    fn render_math<M>(self, renderer: &'a M) -> MathTransformer<'a, Self, M>
    where
        M: MathRenderer,
    {
        MathTransformer {
            inner: self,
            renderer,
        }
    }
}

impl<'a, I> ToMathTransformer<'a> for I where I: Iterator<Item = Event<'a>> {}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use super::*;
    use crate::{
        document::Html,
        math::{escape_html, MathRenderer},
        test_support::{gen_any_utf8, gen_display_mode, DEFAULT_CASES},
    };
    use proptest::{collection, prelude::*};
    use pulldown_cmark::{html, CowStr, Event};
    use std::sync::{Arc, Mutex};

    fn config() -> ProptestConfig {
        ProptestConfig {
            cases: DEFAULT_CASES,
            ..ProptestConfig::default()
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
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = RecordingMathRenderer::new(math_calls.clone());

            let expected_maths = collect_math_calls(&events);
            let transformed: Vec<Event> = events
                .clone()
                .into_iter()
                .render_math(&renderer)
                .collect();

            let recorded = math_calls.lock().expect("math log poisoned");
            prop_assert_eq!(&*recorded, &expected_maths);

            let mut output = String::new();
            html::push_html(&mut output, transformed.into_iter());
            let inline_markers = output.matches("<X-MATH display=false>").count();
            let display_markers = output.matches("<X-MATH display=true>").count();
            let expected_inline = expected_maths.iter().filter(|(_, display)| !*display).count();
            let expected_display = expected_maths.iter().filter(|(_, display)| *display).count();
            prop_assert_eq!(inline_markers, expected_inline);
            prop_assert_eq!(display_markers, expected_display);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn inline_math_renders_correctly(source in gen_any_utf8()) {
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = RecordingMathRenderer::new(math_calls.clone());

            let events = vec![Event::InlineMath(CowStr::from(source.clone()))];
            let transformed: Vec<Event> = events
                .into_iter()
                .render_math(&renderer)
                .collect();

            let recorded = math_calls.lock().expect("math log poisoned");
            prop_assert_eq!(recorded.len(), 1);
            prop_assert_eq!(&recorded[0].0, &source);
            prop_assert_eq!(recorded[0].1, false);

            prop_assert_eq!(transformed.len(), 1);
            prop_assert!(matches!(transformed[0], Event::InlineHtml(_)));
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn display_math_renders_correctly(source in gen_any_utf8()) {
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = RecordingMathRenderer::new(math_calls.clone());

            let events = vec![Event::DisplayMath(CowStr::from(source.clone()))];
            let transformed: Vec<Event> = events
                .into_iter()
                .render_math(&renderer)
                .collect();

            let recorded = math_calls.lock().expect("math log poisoned");
            prop_assert_eq!(recorded.len(), 1);
            prop_assert_eq!(&recorded[0].0, &source);
            prop_assert_eq!(recorded[0].1, true);

            prop_assert_eq!(transformed.len(), 1);
            prop_assert!(matches!(transformed[0], Event::Html(_)));
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn non_math_events_passthrough(
            events in collection::vec(
                prop_oneof![
                    gen_any_utf8().prop_map(|s| Event::Text(CowStr::from(s))),
                    gen_any_utf8().prop_map(|s| Event::Code(CowStr::from(s))),
                    Just(Event::SoftBreak),
                    Just(Event::HardBreak),
                ],
                0..=16
            )
        ) {
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = RecordingMathRenderer::new(math_calls.clone());

            let transformed: Vec<Event> = events
                .clone()
                .into_iter()
                .render_math(&renderer)
                .collect();

            let recorded = math_calls.lock().expect("math log poisoned");
            prop_assert_eq!(recorded.len(), 0);
            prop_assert_eq!(transformed.len(), events.len());

            for (original, transformed) in events.iter().zip(transformed.iter()) {
                match (original, transformed) {
                    (Event::Text(a), Event::Text(b)) => prop_assert_eq!(a, b),
                    (Event::Code(a), Event::Code(b)) => prop_assert_eq!(a, b),
                    (Event::SoftBreak, Event::SoftBreak) => {},
                    (Event::HardBreak, Event::HardBreak) => {},
                    _ => prop_assert!(false, "Event type mismatch"),
                }
            }
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn transformer_completes_on_finite_input(events in gen_math_sequences()) {
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = RecordingMathRenderer::new(math_calls);
            let _transformed: Vec<Event> = events
                .into_iter()
                .render_math(&renderer)
                .collect();
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn display_mode_parameter_passed_correctly(
            source in gen_any_utf8(),
            display_mode in gen_display_mode()
        ) {
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = RecordingMathRenderer::new(math_calls.clone());

            let event = if display_mode {
                Event::DisplayMath(CowStr::from(source.clone()))
            } else {
                Event::InlineMath(CowStr::from(source.clone()))
            };

            let _transformed: Vec<Event> = vec![event]
                .into_iter()
                .render_math(&renderer)
                .collect();

            let recorded = math_calls.lock().expect("math log poisoned");
            prop_assert_eq!(recorded.len(), 1);
            prop_assert_eq!(&recorded[0].0, &source);
            prop_assert_eq!(recorded[0].1, display_mode);
        }
    }

    proptest! {
        #![proptest_config(config())]
        #[test]
        fn mixed_inline_and_display_math(
            inline_sources in collection::vec(gen_any_utf8(), 0..=4),
            display_sources in collection::vec(gen_any_utf8(), 0..=4)
        ) {
            let math_calls = Arc::new(Mutex::new(Vec::new()));
            let renderer = RecordingMathRenderer::new(math_calls.clone());

            let mut events = Vec::new();
            for source in &inline_sources {
                events.push(Event::InlineMath(CowStr::from(source.clone())));
            }
            for source in &display_sources {
                events.push(Event::DisplayMath(CowStr::from(source.clone())));
            }

            let _transformed: Vec<Event> = events
                .into_iter()
                .render_math(&renderer)
                .collect();

            let recorded = math_calls.lock().expect("math log poisoned");
            let expected_count = inline_sources.len() + display_sources.len();
            prop_assert_eq!(recorded.len(), expected_count);

            let inline_count = recorded.iter().filter(|(_, display)| !*display).count();
            let display_count = recorded.iter().filter(|(_, display)| *display).count();
            prop_assert_eq!(inline_count, inline_sources.len());
            prop_assert_eq!(display_count, display_sources.len());
        }
    }
}
