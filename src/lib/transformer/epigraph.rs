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
            // Match Start tag
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

                // i is now at the closing TagEnd::BlockQuote. Check for footer.
                if let Some(footer_text) = extract_footer(&mut buffer) {
                    out.push(start_tag);
                    out.extend(buffer);

                    // Inject <footer> inside the blockquote
                    let footer_html = format!("<footer>{}</footer>", escape_html(&footer_text));
                    out.push(Event::Html(CowStr::from(footer_html)));

                    if i < events.len() {
                        out.push(events[i].clone()); // Close BlockQuote
                    }
                } else {
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

fn extract_footer<'a>(buffer: &mut Vec<Event<'a>>) -> Option<String> {
    // Find the last significant text node.
    let mut text_idx = None;
    for (idx, event) in buffer.iter().enumerate().rev() {
        if let Event::Text(t) = event
            && !t.trim().is_empty()
        {
            text_idx = Some(idx);
            break;
        }
    }

    let idx = text_idx?;
    let text = match &buffer[idx] {
        Event::Text(t) => t,
        _ => return None,
    };

    // Check for delimiters
    let split_pos = text
        .rfind("--")
        .or_else(|| text.rfind('\u{2013}'))
        .or_else(|| text.rfind('\u{2014}'));

    let pos = split_pos?;

    let (content, footer_raw) = text.split_at(pos);

    // Clean up the extracted footer
    let footer_clean = footer_raw
        .trim_start_matches(['-', '\u{2013}', '\u{2014}'])
        .trim()
        .to_string();

    if footer_clean.is_empty() {
        return None;
    }

    // Modify the buffer, truncate or remove the text node
    let new_content = content.trim_end().to_string();
    if new_content.is_empty() {
        buffer.remove(idx);
    } else {
        buffer[idx] = Event::Text(CowStr::from(new_content));
    }

    // Cleanup empty paragraph wrapper if we emptied the text
    cleanup_empty_paragraph(buffer);

    Some(footer_clean)
}

fn cleanup_empty_paragraph(buffer: &mut Vec<Event>) {
    // Remove trailing whitespace/breaks
    while let Some(last) = buffer.last() {
        match last {
            Event::Text(t) if t.trim().is_empty() => {
                buffer.pop();
            }
            Event::SoftBreak | Event::HardBreak => {
                buffer.pop();
            }
            _ => break,
        }
    }

    // If the last thing is End(P), check if it matches a Start(P) with no
    // content in between
    if let Some(Event::End(TagEnd::Paragraph)) = buffer.last() {
        let mut p_start = None;
        for i in (0..buffer.len() - 1).rev() {
            match &buffer[i] {
                Event::Start(Tag::Paragraph) => {
                    p_start = Some(i);
                    break;
                }
                Event::End(_) => break, // Nested structure, abort
                // Real content, abort
                Event::Text(t) if !t.trim().is_empty() => break,
                Event::Code(_)
                | Event::Html(_)
                | Event::InlineHtml(_)
                | Event::Start(Tag::Image { .. }) => {
                    break;
                }
                _ => {}
            }
        }

        if let Some(start) = p_start {
            buffer.drain(start..);
        }
    }
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
