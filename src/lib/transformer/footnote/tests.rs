use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use crate::transformer::footnote::convert_footnotes_to_sidenotes;

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
