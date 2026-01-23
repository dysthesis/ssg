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

                // Check for footer in the captured buffer
                if let Some(footer_text) = extract_footer(&mut buffer) {
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

fn extract_footer<'a>(buffer: &mut Vec<Event<'a>>) -> Option<String> {
    // Find the index of the last *significant* text event. We skip trailing
    // whitespace or softbreaks to find the actual content.
    let mut text_idx = None;
    for (idx, event) in buffer.iter().enumerate().rev() {
        if let Event::Text(t) = event {
            if !t.trim().is_empty() {
                text_idx = Some(idx);
                break;
            }
        }
    }

    let idx = text_idx?;
    let text = match &buffer[idx] {
        Event::Text(t) => t,
        _ => return None,
    };

    // Check for delimiters in that text node. Smart punctuation might have
    // converted "--" into En-Dash (\u{2013}) or Em-Dash (\u{2014}).
    let split_pos = text
        .rfind("--")
        .or_else(|| text.rfind('\u{2013}'))
        .or_else(|| text.rfind('\u{2014}'));

    let Some(pos) = split_pos else {
        return None;
    };

    let (content, footer_raw) = text.split_at(pos);

    // Verify the footer looks like an attribution
    let footer_clean = footer_raw
        .chars()
        .skip_while(|c| *c == '-' || *c == '\u{2013}' || *c == '\u{2014}')
        .collect::<String>()
        .trim()
        .to_string();

    if footer_clean.is_empty() {
        return None;
    }

    // Modify the text event in the buffer
    let remaining_content = content.trim_end().to_string();

    if remaining_content.is_empty() {
        // If the text node contained ONLY the footer, remove it entirely.
        buffer.remove(idx);
    } else {
        // Otherwise, keep the content part.
        buffer[idx] = Event::Text(CowStr::from(remaining_content));
    }

    // Cleanup: If we removed the text and left an empty paragraph wrapper,
    // remove the wrapper too.
    cleanup_empty_paragraph(buffer);

    Some(footer_clean)
}

/// Removes a trailing empty paragraph from the buffer.
/// E.g. turns `[..., Start(P), End(P)]` into `[...]`.
fn cleanup_empty_paragraph(buffer: &mut Vec<Event>) {
    // First, remove any trailing whitespace/breaks that might be sitting after
    // the text we just removed.
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

    // Now check if the last element is End(Paragraph)
    if let Some(Event::End(TagEnd::Paragraph)) = buffer.last() {
        // Search backwards for the matching Start(Paragraph)
        // If we find it without encountering any "real" content, we delete the range.
        let mut p_start = None;
        for i in (0..buffer.len() - 1).rev() {
            match &buffer[i] {
                Event::Start(Tag::Paragraph) => {
                    p_start = Some(i);
                    break;
                }
                // If we hit another End tag (nested structure) or content, stop.
                Event::End(_) => break,
                Event::Text(t) if !t.trim().is_empty() => break,
                Event::Code(_) | Event::Html(_) | Event::Start(Tag::Image { .. }) => break,
                _ => {} // Continue past whitespace, softbreaks, etc.
            }
        }

        if let Some(start) = p_start {
            // Remove the empty paragraph events
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
