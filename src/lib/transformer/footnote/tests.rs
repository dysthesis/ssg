use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use crate::transformer::footnote::{
    convert_footnotes_to_plain_list, convert_footnotes_to_sidenotes,
};

#[test]
fn footnote_transformer_inlines_definition() {
    let events = vec![
        Event::FootnoteReference(CowStr::from("a")),
        Event::Start(Tag::FootnoteDefinition(CowStr::from("a"))),
        Event::Start(Tag::Paragraph),
        Event::Text(CowStr::from("hello")),
        Event::End(TagEnd::Paragraph),
        Event::End(TagEnd::FootnoteDefinition),
    ];

    let out = convert_footnotes_to_sidenotes(events);

    assert!(out.iter().any(|e| matches!(e, Event::InlineHtml(_))));
    assert!(!out.iter().any(|e| matches!(e, Event::FootnoteReference(_))));
    assert!(
        !out.iter()
            .any(|e| matches!(e, Event::Start(Tag::FootnoteDefinition(_))))
    );
}

#[test]
fn plain_transformer_renders_ordered_list() {
    let events = vec![
        Event::Text(CowStr::from("see note")),
        Event::FootnoteReference(CowStr::from("a")),
        Event::Start(Tag::FootnoteDefinition(CowStr::from("a"))),
        Event::Start(Tag::Paragraph),
        Event::Text(CowStr::from("first footnote")),
        Event::End(TagEnd::Paragraph),
        Event::End(TagEnd::FootnoteDefinition),
    ];

    let out = convert_footnotes_to_plain_list(events);
    let joined = out
        .iter()
        .map(|e| match e {
            Event::Html(s) | Event::InlineHtml(s) => s.to_string(),
            Event::Text(s) => s.to_string(),
            _ => String::new(),
        })
        .collect::<String>();

    assert!(joined.contains("footnotes"));
    assert!(joined.contains("<ol>"));
    assert!(joined.contains("first footnote"));
    assert!(joined.contains("fnref-1"));
    assert!(!joined.contains("margin-toggle"));
}
