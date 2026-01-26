use pulldown_cmark::{CowStr, Event, HeadingLevel, Tag, TagEnd};

use crate::{
    transformer::Transformer,
    utils::{escape_attr, escape_text, slugify},
};

pub struct TocTransformer<'a> {
    inner: std::vec::IntoIter<Event<'a>>,
}

impl<'a> Iterator for TocTransformer<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a, I> Transformer<'a, I> for TocTransformer<'a>
where
    I: Iterator<Item = Event<'a>>,
{
    fn transform(inner: I) -> Self {
        let events: Vec<Event<'a>> = inner.collect();
        let rewritten = insert_toc_and_heading_ids(events);
        Self {
            inner: rewritten.into_iter(),
        }
    }
}

/// Insert a margin TOC (based on h2 and h3) and assign ids to headings when absent.
///
/// With your current pipeline, Markdown `#` becomes HTML `h2`, and `##` becomes `h3`.
pub fn insert_toc_and_heading_ids<'a>(events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    let TocExtraction {
        events: body,
        headings,
    } = extract_headings(events);

    if headings.is_empty() {
        return body;
    }

    let toc_html = build_toc_html(&headings);
    let mut final_out: Vec<Event<'a>> = Vec::with_capacity(body.len() + 1);
    final_out.push(Event::Html(CowStr::from(toc_html)));
    final_out.extend(body);
    final_out
}

fn build_toc_html(headings: &[HeadingEntry]) -> String {
    use std::fmt::Write as _;

    let mut h2_n: usize = 0;
    let mut h3_n: usize = 0;

    let mut li_open = false;

    let mut sub_open = false;

    let mut s = String::new();
    // Anchor constrains percentage margins to the text column width.
    s.push_str(r#"<div class="toc-anchor">"#);

    s.push_str(r#"<nav class="toc marginnote" aria-label="Contents">"#);
    s.push_str(r#"<p class="toc-title">Contents</p>"#);
    s.push_str(r#"<ol class="toc-list">"#);
    for (i, entry) in headings.iter().enumerate() {
        let next_level = headings.get(i + 1).map(|h| h.level);

        if matches!(entry.level, HeadingLevel::H2) {
            if li_open {
                if sub_open {
                    s.push_str("</ol>");
                    sub_open = false;
                }
                s.push_str("</li>");
            }

            li_open = true;
            h2_n += 1;
            h3_n = 0;

            let num = format!("{:02}", h2_n);
            let href_id = escape_attr(&entry.id);
            let text = escape_text(&entry.title);

            s.push_str(r#"<li class="toc-l1">"#);
            write!(&mut s, r##"<a href="#{}">"##, href_id).unwrap();
            s.push_str(r#"<span class="toc-num">"#);
            s.push_str(&num);
            s.push_str(r#"</span>"#);
            s.push_str(r#"<span class="toc-text">"#);
            s.push_str(&text);
            s.push_str(r#"</span><span class="toc-leader" aria-hidden="true"></span></a>"#);

            if matches!(next_level, Some(HeadingLevel::H3)) {
                s.push_str(r#"<ol class="toc-sub">"#);
                sub_open = true;
            }
        } else if matches!(entry.level, HeadingLevel::H3) {
            // If an h3 appears without a preceding h2, render it as a standalone top-level item.
            if !li_open {
                h2_n += 1;
                h3_n = 0;

                let num = format!("{:02}", h2_n);
                let href_id = escape_attr(&entry.id);
                let text = escape_text(&entry.title);

                s.push_str(r#"<li class="toc-l1">"#);
                write!(&mut s, r##"<a href="#{}">"##, href_id).unwrap();
                s.push_str(r#"<span class="toc-num">"#);
                s.push_str(&num);
                s.push_str(r#"</span>"#);
                s.push_str(r#"<span class="toc-text">"#);
                s.push_str(&text);
                s.push_str(
                    r#"</span><span class="toc-leader" aria-hidden="true"></span></a></li>"#,
                );
                continue;
            }

            h3_n += 1;
            let num = format!("{:02}.{}", h2_n, h3_n);

            let href_id = escape_attr(&entry.id);
            let text = escape_text(&entry.title);

            s.push_str(r#"<li class="toc-l2">"#);
            write!(&mut s, r##"<a href="#{}">"##, href_id).unwrap();
            s.push_str(r#"<span class="toc-num">"#);
            s.push_str(&num);
            s.push_str(r#"</span>"#);
            s.push_str(r#"<span class="toc-text">"#);
            s.push_str(&text);
            s.push_str(r#"</span><span class="toc-leader" aria-hidden="true"></span></a></li>"#);
        }
    }

    if li_open {
        if sub_open {
            s.push_str("</ol>");
        }
        s.push_str("</li>");
    }

    s.push_str("</ol></nav>");
    s.push_str("</div>");
    s
}

#[derive(Debug)]
struct HeadingEntry {
    level: HeadingLevel,
    id: String,
    title: String,
}

struct TocExtraction<'a> {
    events: Vec<Event<'a>>,
    headings: Vec<HeadingEntry>,
}

fn extract_headings<'a>(events: Vec<Event<'a>>) -> TocExtraction<'a> {
    let mut out: Vec<Event<'a>> = Vec::with_capacity(events.len() + 1);
    let mut headings: Vec<HeadingEntry> = Vec::new();
    let mut slug_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    // Track current heading: (level, start_index_in_out, collected_title, existing_id)
    let mut in_heading: Option<(HeadingLevel, usize, String, Option<String>)> = None;

    for ev in events {
        match (&mut in_heading, ev) {
            (
                None,
                Event::Start(Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                }),
            ) if matches!(level, HeadingLevel::H2 | HeadingLevel::H3) => {
                let start_index = out.len();
                let existing_id = id.as_ref().map(|c| c.to_string());

                // Write a placeholder start tag; we will patch in the id at the end tag.
                out.push(Event::Start(Tag::Heading {
                    level,
                    id: None,
                    classes,
                    attrs,
                }));

                in_heading = Some((level, start_index, String::new(), existing_id));
            }

            (Some((_, _, title_buf, _)), Event::Text(t)) => {
                title_buf.push_str(t.as_ref());
                out.push(Event::Text(t));
            }

            (Some((_, _, title_buf, _)), Event::Code(t)) => {
                title_buf.push_str(t.as_ref());
                out.push(Event::Code(t));
            }

            (
                Some((level, start_index, title_buf, existing_id)),
                Event::End(TagEnd::Heading(_end)),
            ) => {
                let title = title_buf.trim().to_string();

                let base = existing_id.clone().unwrap_or_else(|| slugify(&title));
                let unique = uniquify_slug(base, &mut slug_counts);

                // Patch the start event with the final id.
                let old = std::mem::replace(&mut out[*start_index], Event::Text(CowStr::from("")));
                out[*start_index] = match old {
                    Event::Start(Tag::Heading {
                        level,
                        classes,
                        attrs,
                        ..
                    }) => Event::Start(Tag::Heading {
                        level,
                        id: Some(CowStr::from(unique.clone())),
                        classes,
                        attrs,
                    }),
                    other => other,
                };

                headings.push(HeadingEntry {
                    level: *level,
                    id: unique,
                    title,
                });

                out.push(Event::End(TagEnd::Heading(*level)));
                in_heading = None;
            }

            // Any other event while in a heading: forward it, but do not add to title text.
            (Some(_), other) => out.push(other),

            // Not in heading: pass through unchanged.
            (None, other) => out.push(other),
        }
    }

    TocExtraction {
        events: out,
        headings,
    }
}
fn uniquify_slug(base: String, counts: &mut std::collections::HashMap<String, usize>) -> String {
    let n = counts.entry(base.clone()).or_insert(0);
    *n += 1;

    if *n == 1 {
        base
    } else {
        format!("{base}-{}", *n)
    }
}

// escape functions provided by utils
