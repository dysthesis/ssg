use crate::transformer::Transformer;
use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use std::collections::HashMap;

pub struct FootnoteTransformer<'a> {
    inner: std::vec::IntoIter<Event<'a>>,
}

impl<'a> Iterator for FootnoteTransformer<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a, I> Transformer<'a, I> for FootnoteTransformer<'a>
where
    I: Iterator<Item = Event<'a>>,
{
    fn transform(inner: I) -> Self {
        let events: Vec<Event<'a>> = inner.collect();
        let rewritten = convert_footnotes_to_sidenotes(events);
        Self {
            inner: rewritten.into_iter(),
        }
    }
}

pub fn convert_footnotes_to_sidenotes<'a>(events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    let defs = collect_definitions(&events);

    let mut out: Vec<Event<'a>> = Vec::with_capacity(events.len());
    let mut skipping_definition_depth: usize = 0;
    let mut sidenote_index: usize = 0;

    for event in events {
        if skipping_definition_depth > 0 {
            match event {
                Event::Start(_) => skipping_definition_depth += 1,
                Event::End(_) => {
                    skipping_definition_depth = skipping_definition_depth.saturating_sub(1)
                }
                _ => {}
            }
            continue;
        }

        match event {
            Event::Start(Tag::FootnoteDefinition(_label)) => {
                // Skip rendering definitions at the bottom.
                skipping_definition_depth = 1;
            }

            Event::FootnoteReference(label) => {
                sidenote_index += 1;
                let id = format!("sn-{sidenote_index}");
                let display = sidenote_index;

                let def_events = defs
                    .get(label.as_ref())
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);

                let def_html = render_definition_as_inline_html(def_events);

                let html = format!(
                    r#"<label for="{id}" class="margin-toggle sidenote-number" data-sidenote="{display}"></label><input type="checkbox" id="{id}" class="margin-toggle"/><span class="sidenote" data-sidenote="{display}">{def_html}</span>"#
                );

                out.push(Event::InlineHtml(CowStr::from(html)));
            }

            other => out.push(other),
        }
    }

    out
}

fn collect_definitions<'a>(events: &[Event<'a>]) -> HashMap<String, Vec<Event<'a>>> {
    let mut defs: HashMap<String, Vec<Event<'a>>> = HashMap::new();

    let mut i: usize = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::FootnoteDefinition(label)) => {
                let key = label.to_string();

                // Capture everything inside this definition block.
                let mut depth: usize = 1;
                let mut inner: Vec<Event<'a>> = Vec::new();

                i += 1;
                while i < events.len() && depth > 0 {
                    match &events[i] {
                        Event::Start(_) => {
                            depth += 1;
                            inner.push(events[i].clone());
                        }
                        Event::End(_) => {
                            depth = depth.saturating_sub(1);
                            if depth > 0 {
                                inner.push(events[i].clone());
                            }
                        }
                        other => inner.push(other.clone()),
                    }
                    i += 1;
                }

                defs.insert(key, inner);
                continue;
            }
            _ => i += 1,
        }
    }

    defs
}

/// Render a footnote definition in a way that is safe inside `<span class="sidenote">…</span>`.
///
/// This removes block-level tags (`<p>`, `<blockquote>`) and replaces their structure with
/// inline HTML (`<br>`, `<span class="sidenote-quote">…</span>`).
fn render_definition_as_inline_html<'a>(events: &[Event<'a>]) -> String {
    let inline_events = inlineify_definition_events(events);

    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, inline_events.into_iter());

    html.trim().to_string()
}

fn inlineify_definition_events<'a>(events: &[Event<'a>]) -> Vec<Event<'a>> {
    let mut out: Vec<Event<'a>> = Vec::with_capacity(events.len());

    let mut at_start = true;
    let mut pending_paragraph_break = false;

    for ev in events.iter().cloned() {
        match ev {
            // Paragraphs are block-level; drop the tags and insert breaks
            // between them.
            Event::Start(Tag::Paragraph) => {
                if !at_start && (pending_paragraph_break || !out.is_empty()) {
                    out.push(Event::InlineHtml(CowStr::from("<br><br>")));
                }
                pending_paragraph_break = false;
                at_start = false;
            }
            Event::End(TagEnd::Paragraph) => {
                pending_paragraph_break = true;
            }

            // Block quotes are block-level; replace with an inline span wrapper.
            Event::Start(Tag::BlockQuote(_)) => {
                if !at_start {
                    out.push(Event::InlineHtml(CowStr::from("<br><br>")));
                }
                out.push(Event::InlineHtml(CowStr::from(
                    r#"<span class="sidenote-quote">"#,
                )));
                pending_paragraph_break = false;
                at_start = false;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                out.push(Event::InlineHtml(CowStr::from("</span>")));
                pending_paragraph_break = true;
                at_start = false;
            }

            // Keep line breaks as line breaks.
            Event::SoftBreak | Event::HardBreak => {
                out.push(Event::InlineHtml(CowStr::from("<br>")));
                pending_paragraph_break = false;
                at_start = false;
            }

            // Avoid recursive sidenotes inside sidenotes
            Event::FootnoteReference(_label) => {
                // Either drop, or render a literal marker.
            }

            other => {
                out.push(other);
                pending_paragraph_break = false;
                at_start = false;
            }
        }
    }

    out
}
