use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use crate::transformer::{WithTransformer, code_block::CodeHighlightTransformer};

#[test]
fn code_highlight_replaces_block() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(&".*", |body| {
            let events = vec![
                Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(
                    CowStr::from("rs"),
                ))),
                Event::Text(CowStr::from(body.clone())),
                Event::End(TagEnd::CodeBlock),
            ];
            let out: Vec<_> = events
                .into_iter()
                .with_transformer::<CodeHighlightTransformer<_>>()
                .collect();
            prop_assert_eq!(out.len(), 1);
            prop_assert!(matches!(out[0], Event::Html(_)));
            Ok(())
        })
        .unwrap();
}
