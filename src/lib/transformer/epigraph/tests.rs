use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use crate::transformer::{epigraph::EpigraphTransformer, Transformer};

#[test]
fn epigraph_transformer_detects_final_attribution() {
    let events = vec![
        Event::Start(Tag::BlockQuote(None)),
        Event::Start(Tag::Paragraph),
        Event::Text(CowStr::from("This is the quote.")),
        Event::End(TagEnd::Paragraph),
        Event::Start(Tag::Paragraph),
        Event::Text(CowStr::from("—Author")),
        Event::End(TagEnd::Paragraph),
        Event::End(TagEnd::BlockQuote(None)),
    ];

    let out: Vec<_> = EpigraphTransformer::transform(events.into_iter()).collect();
    let html = out
        .iter()
        .filter_map(|e| match e {
            Event::Html(h) | Event::InlineHtml(h) => Some(h.to_string()),
            _ => None,
        })
        .collect::<String>();

    assert!(html.contains(r#"<div class="epigraph">"#));
}
