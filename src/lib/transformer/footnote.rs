use crate::transformer::Transformer;
use pulldown_cmark::{CowStr, Event, Tag};
use std::collections::HashMap;

/// Transformer to modify footnotes into sidenotes for tufte.css
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

/// Convert Markdown footnotes (endnotes) into Tufte-style inline sidenotes.
///
/// This rewrites the event stream:
/// - Collects all footnote definitions into a map.
/// - Replaces each footnote reference with Tufte HTML inserted inline.
/// - Removes the original footnote-definition blocks from the output.
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
                // Skip the entire definition block (including nested tags) so
                // it does not render at the bottom.
                skipping_definition_depth = 1;
            }

            Event::FootnoteReference(label) => {
                sidenote_index += 1;
                let id = format!("sn-{sidenote_index}");

                let def_html = defs
                    .get(label.as_ref())
                    .map(|s| inlineify_footnote_html(s))
                    .unwrap_or_default();

                let html = format!(
                    r#"<label for="{id}" class="margin-toggle sidenote-number"></label><input type="checkbox" id="{id}" class="margin-toggle"/><span class="sidenote">{def_html}</span>"#
                );

                out.push(Event::InlineHtml(CowStr::from(html)));
            }

            other => out.push(other),
        }
    }

    out
}

fn collect_definitions<'a>(events: &[Event<'a>]) -> HashMap<String, String> {
    let mut defs: HashMap<String, String> = HashMap::new();

    let mut i: usize = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::FootnoteDefinition(label)) => {
                let key = label.to_string();

                // Capture everything inside this definition block by tracking
                // nested Start/End events until the depth returns to zero.
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

                let mut html = String::new();
                pulldown_cmark::html::push_html(&mut html, inner.into_iter());
                defs.insert(key, html);

                continue;
            }
            _ => {
                i += 1;
            }
        }
    }

    defs
}

fn inlineify_footnote_html(html: &str) -> String {
    let mut s = html.trim().to_string();

    // Convert paragraph boundaries to line breaks.
    s = s.replace("</p>\n<p>", "<br><br>");
    s = s.replace("</p><p>", "<br><br>");

    // Remove a single outer paragraph wrapper if present.
    if let Some(rest) = s.strip_prefix("<p>") {
        s = rest.to_string();
    }
    if let Some(rest) = s.strip_suffix("</p>") {
        s = rest.to_string();
    }

    s
}
