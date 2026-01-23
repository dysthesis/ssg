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
                let start_tag = events[i].clone();
                i += 1;

                // Capture the blockquote content
                let mut nesting = 1;
                let mut buffer = Vec::new();

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

                // i is now at the closing TagEnd::BlockQuote. Check if the last
                // text node has the delimiter
                if let Some(footer_text) = extract_and_strip_footer(&mut buffer) {
                    out.push(start_tag);
                    // Push the modified body
                    out.extend(buffer);
                    // Inject the footer element *inside* the blockquote
                    let footer_html = format!("<footer>{}</footer>", escape_html(&footer_text));
                    out.push(Event::Html(CowStr::from(footer_html)));

                    if i < events.len() {
                        out.push(events[i].clone()); // Close BlockQuote
                    }
                } else {
                    // Not an epigraph, render normally
                    out.push(start_tag);
                    out.extend(buffer);
                    if i < events.len() {
                        out.push(events[i].clone());
                    }
                }
            }
            other => out.push(other.clone()),
        }
        i += 1;
    }

    out
}

/// Looks at the end of the event buffer for "--".
/// If found, modifies the buffer to remove the footer from the text event,
/// and returns the footer string.
fn extract_and_strip_footer<'a>(events: &mut Vec<Event<'a>>) -> Option<String> {
    // Iterate backwards to find the last text node
    for idx in (0..events.len()).rev() {
        if let Event::Text(text) = &events[idx] {
            // Check for standard dash, En-dash, or Em-dash (smart punctuation)
            let split_pos = text
                .rfind("--")
                .or_else(|| text.rfind('\u{2013}'))
                .or_else(|| text.rfind('\u{2014}'));

            if let Some(pos) = split_pos {
                let (content, footer) = text.split_at(pos);

                // Clean the footer text
                let clean_footer = footer
                    .chars()
                    .skip_while(|c| *c == '-' || *c == '\u{2013}' || *c == '\u{2014}')
                    .collect::<String>()
                    .trim()
                    .to_string();

                if clean_footer.is_empty() {
                    continue;
                }

                // Modify the buffer. Truncate the text event
                let content_str = content.trim_end().to_string();
                if content_str.is_empty() {
                    events.remove(idx);
                } else {
                    events[idx] = Event::Text(CowStr::from(content_str));
                }

                return Some(clean_footer);
            }

            // If the last text node doesn't have it, we stop looking.
            return None;
        }
    }
    None
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}
