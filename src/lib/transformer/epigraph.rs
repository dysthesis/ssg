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
                let (block, consumed) = capture_blockquote(&events[i..]);
                i += consumed;

                if let Some(epigraph) = block.and_then(EpigraphBlock::from_events) {
                    render_epigraph(&mut out, epigraph);
                } else {
                    // Fallback: emit original blockquote content as-is
                    out.push(Event::Start(Tag::BlockQuote(None)));
                    out.extend(events[i - consumed + 1..i - 1].iter().cloned());
                    out.push(Event::End(TagEnd::BlockQuote(None)));
                }
            }
            other => {
                out.push(other.clone());
                i += 1;
            }
        }
    }

    out
}

/// Captures a blockquote starting at index 0; returns its inner events and items consumed.
fn capture_blockquote<'a>(slice: &[Event<'a>]) -> (Option<Vec<Event<'a>>>, usize) {
    if !matches!(slice.first(), Some(Event::Start(Tag::BlockQuote(_)))) {
        return (None, 0);
    }

    let mut buffer = Vec::new();
    let mut nesting = 1;
    let mut i = 1;
    while i < slice.len() && nesting > 0 {
        match &slice[i] {
            Event::Start(Tag::BlockQuote(_)) => nesting += 1,
            Event::End(TagEnd::BlockQuote(_)) => nesting -= 1,
            _ => {}
        }
        if nesting > 0 {
            buffer.push(slice[i].clone());
        }
        i += 1;
    }

    (Some(buffer), i)
}

#[derive(Debug)]
struct EpigraphBlock<'a> {
    quote: Vec<Event<'a>>,
    attribution: Vec<Event<'a>>,
}

impl<'a> EpigraphBlock<'a> {
    fn from_events(events: Vec<Event<'a>>) -> Option<Self> {
        let last_para_start = events
            .iter()
            .rposition(|e| matches!(e, Event::Start(Tag::Paragraph)))?;

        let attribution = events[last_para_start..].to_vec();
        let quote = events[..last_para_start].to_vec();

        if !is_epigraph(&attribution) {
            return None;
        }

        Some(Self { quote, attribution })
    }
}

fn is_epigraph(buffer: &[Event]) -> bool {
    buffer.iter().any(|event| {
        if let Event::Text(text) = event {
            let s = text.trim();
            s.starts_with('—') || s.starts_with("--")
        } else {
            false
        }
    })
}

fn render_epigraph<'a>(out: &mut Vec<Event<'a>>, block: EpigraphBlock<'a>) {
    // Open container
    out.push(Event::Html(CowStr::from(r#"<div class="epigraph">"#)));
    out.push(Event::Html(CowStr::from("\n")));

    // 1. Render the quote body
    out.push(Event::Html(CowStr::from(r#"<blockquote>"#)));
    out.extend(block.quote);
    out.push(Event::Html(CowStr::from(r#"</blockquote>"#)));

    // 2. Render the attribution
    out.push(Event::Html(CowStr::from(r#"<p class="attribution">"#)));

    for event in block.attribution {
        match event {
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => continue,
            Event::Text(t) => {
                let s = t.trim_start_matches('—').trim_start_matches('-');
                out.push(Event::Text(CowStr::from(s.to_owned())));
            }
            other => out.push(other),
        }
    }

    out.push(Event::Html(CowStr::from(r#"</p>"#)));
    out.push(Event::Html(CowStr::from(r#"</div>"#)));
    out.push(Event::Html(CowStr::from("\n")));
}
