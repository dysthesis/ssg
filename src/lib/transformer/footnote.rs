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

    // Whether the *next* paragraph in the current container needs a separator.
    // Index 0 is the top-level footnote definition container.
    let mut need_par_sep_stack: Vec<bool> = vec![false];

    let mut quote_depth: usize = 0;
    let mut last_was_break: bool = false;

    let push_break = |out: &mut Vec<Event<'a>>, html: &'static str, last_was_break: &mut bool| {
        if !*last_was_break {
            out.push(Event::InlineHtml(CowStr::from(html)));
            *last_was_break = true;
        }
    };

    for ev in events.iter().cloned() {
        match ev {
            // Drop paragraph tags; insert separators between paragraphs.
            Event::Start(Tag::Paragraph) => {
                if *need_par_sep_stack.last().unwrap_or(&false) {
                    if quote_depth > 0 {
                        push_break(&mut out, "<br>", &mut last_was_break);
                    } else {
                        push_break(&mut out, "<br><br>", &mut last_was_break);
                    }
                    if let Some(top) = need_par_sep_stack.last_mut() {
                        *top = false;
                    }
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if let Some(top) = need_par_sep_stack.last_mut() {
                    *top = true;
                }
                last_was_break = false;
            }

            // Replace blockquote with an inline wrapper.
            Event::Start(Tag::BlockQuote(_)) => {
                if !out.is_empty() {
                    push_break(&mut out, "<br><br>", &mut last_was_break);
                }
                out.push(Event::InlineHtml(CowStr::from(
                    r#"<span class="sidenote-quote">"#,
                )));
                quote_depth += 1;
                need_par_sep_stack.push(false);
                last_was_break = false;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                out.push(Event::InlineHtml(CowStr::from("</span>")));
                quote_depth = quote_depth.saturating_sub(1);
                need_par_sep_stack.pop();
                if let Some(top) = need_par_sep_stack.last_mut() {
                    *top = true;
                }
                last_was_break = false;
            }

            // HardBreak is an explicit line break; SoftBreak should be a space.
            Event::HardBreak => {
                push_break(&mut out, "<br>", &mut last_was_break);
            }
            Event::SoftBreak => {
                out.push(Event::Text(CowStr::from(" ")));
                last_was_break = false;
            }

            // Rewrite raw HTML that is invalid inside <span>.
            Event::Html(s) => {
                out.push(Event::InlineHtml(rewrite_sidenote_html(s)));
                last_was_break = false;
            }
            Event::InlineHtml(s) => {
                out.push(Event::InlineHtml(rewrite_sidenote_html(s)));
                last_was_break = false;
            }

            // Avoid recursive footnote references inside footnote bodies.
            Event::FootnoteReference(_) => {}

            other => {
                out.push(other);
                last_was_break = false;
            }
        }
    }

    out
}

fn rewrite_sidenote_html<'a>(s: CowStr<'a>) -> CowStr<'a> {
    let raw = s.as_ref();

    // Minimal, targeted sanitisation for your current content.
    if !raw.contains("<footer") && !raw.contains("</footer>") {
        return s;
    }

    // If you ever add attributes to <footer>, this can be made more robust,
    // but this matches your present usage.
    let mut out = raw.replace("<footer>", r#"<span class="sidenote-cite">"#);
    out = out.replace("</footer>", "</span>");
    CowStr::from(out)
}
