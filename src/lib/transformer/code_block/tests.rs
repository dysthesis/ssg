use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use crate::transformer::{
    WithTransformer,
    code_block::{CodeHighlightTransformer, FeedCodeLabelTransformer},
};

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

#[test]
fn feed_code_labels_language() {
    let events = vec![
        Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(
            CowStr::from("go"),
        ))),
        Event::Text(CowStr::from("fmt.Println(\"hi\")")),
        Event::End(TagEnd::CodeBlock),
    ];

    let out: Vec<_> = events
        .into_iter()
        .with_transformer::<FeedCodeLabelTransformer<_>>()
        .collect();

    assert_eq!(out.len(), 1);
    if let Event::Html(html) = &out[0] {
        let s = html.to_string();
        assert!(s.contains("class=\"language-go\""));
        assert!(s.contains("data-lang=\"go\""));
        assert!(s.contains("fmt.Println"));
    } else {
        panic!("expected html");
    }
}
