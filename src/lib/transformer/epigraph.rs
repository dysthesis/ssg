use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use crate::transformer::Transformer;

pub struct EpigraphTransformer<'a> {
    inner: std::vec::IntoIter<Event<'a>>,
}

impl<'a> Iterator for EpigraphTransformer<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a, I> Transformer<'a, I> for EpigraphTransformer<'a>
where
    I: Iterator<Item = Event<'a>>,
{
    fn transform(inner: I) -> Self {
        let events: Vec<Event<'a>> = inner.collect();
        let rewritten = process_epigraphs(events);
        Self {
            inner: rewritten.into_iter(),
        }
    }
}

fn process_epigraphs<'a>(events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    let mut out = Vec::with_capacity(events.len());
    let mut i = 0;

    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::BlockQuote(_)) => {
                let mut buffer = Vec::new();
                let mut nesting = 1;
                i += 1;
                while i < events.len() && nesting > 0 {
                    match &events[i] {
                        Event::Start(Tag::BlockQuote(_)) => nesting += 1,
                        Event::End(TagEnd::BlockQuote(_)) => nesting -= 1,
                        _ => {}
                    }
                    if nesting > 0 {
                        buffer.push(events[i].clone());
                        i += 1;
                    }
                }

                if is_epigraph(&buffer) {
                    render_epigraph(&mut out, buffer);
                } else {
                    out.push(Event::Start(Tag::BlockQuote(None)));
                    out.extend(buffer);
                    out.push(Event::End(TagEnd::BlockQuote(None)));
                }

                i += 1;
            }
            other => {
                out.push(other.clone());
                i += 1;
            }
        }
    }

    out
}

fn is_epigraph(buffer: &[Event]) -> bool {
    let last_para_start = buffer
        .iter()
        .rposition(|e| matches!(e, Event::Start(Tag::Paragraph)));

    if let Some(idx) = last_para_start {
        for event in &buffer[idx..] {
            if let Event::Text(text) = event {
                let s = text.trim();
                return s.starts_with('—') || s.starts_with("--");
            }
        }
    }

    false
}

fn render_epigraph<'a>(out: &mut Vec<Event<'a>>, buffer: Vec<Event<'a>>) {
    // Open container
    out.push(Event::Html(CowStr::from(r#"<div class="epigraph">"#)));
    out.push(Event::Html(CowStr::from("\n")));

    let last_para_start = buffer
        .iter()
        .rposition(|e| matches!(e, Event::Start(Tag::Paragraph)))
        .unwrap();

    // 1. Render the quote body
    out.push(Event::Html(CowStr::from(r#"<blockquote>"#)));
    out.extend(buffer[0..last_para_start].iter().cloned());
    out.push(Event::Html(CowStr::from(r#"</blockquote>"#)));

    // 2. Render the attribution
    out.push(Event::Html(CowStr::from(r#"<p class="attribution">"#)));

    let attribution_content = &buffer[last_para_start..];

    for event in attribution_content {
        match event {
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {
                continue;
            }
            Event::Text(t) => {
                let s = t.trim_start_matches('—').trim_start_matches('-');
                // Convert the resulting String into a CowStr
                out.push(Event::Text(CowStr::from(s.to_owned())));
            }
            _ => out.push(event.clone()),
        }
    }

    out.push(Event::Html(CowStr::from(r#"</p>"#)));
    out.push(Event::Html(CowStr::from(r#"</div>"#)));
    out.push(Event::Html(CowStr::from("\n")));
}
