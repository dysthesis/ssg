use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use std::{fmt::Write, path::Path};

use crate::{transformer::Transformer, utils::escape_attr};

pub struct ImageCaptionTransformer<I> {
    inner: I,
    seen_first: bool,
}

impl<'a, I> Iterator for ImageCaptionTransformer<I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let event = self.inner.next()?;

        match event {
            Event::Start(Tag::Image {
                link_type: _,
                dest_url,
                title,
                id: _,
            }) => {
                let is_first_image = !self.seen_first;
                self.seen_first = true;

                let mut alt_events = Vec::new();
                let mut nesting = 0;

                loop {
                    match self.inner.next() {
                        Some(Event::End(TagEnd::Image)) if nesting == 0 => break,
                        Some(e) => {
                            if let Event::Start(Tag::Image { .. }) = &e {
                                nesting += 1;
                            } else if let Event::End(TagEnd::Image) = &e {
                                nesting -= 1;
                            }
                            alt_events.push(e);
                        }
                        None => break,
                    }
                }

                let mut caption_html = String::new();
                pulldown_cmark::html::push_html(&mut caption_html, alt_events.iter().cloned());

                let mut alt_text = String::new();
                for e in &alt_events {
                    match e {
                        Event::Text(t) | Event::Code(t) => alt_text.push_str(t),
                        _ => {}
                    }
                }

                let dimensions = image_dimensions(&dest_url);
                let size_attrs = dimensions
                    .map(|(w, h)| format!(r#" width="{}" height="{}""#, w, h))
                    .unwrap_or_default();

                let srcset_attrs = dimensions
                    .map(|(w, _)| {
                        format!(
                            r#" srcset="{} {}w" sizes="(max-width: 760px) 92vw, 55vw""#,
                            escape_attr(&dest_url),
                            w
                        )
                    })
                    .unwrap_or_default();

                let loading_attr = if is_first_image { "eager" } else { "lazy" };
                let fetchpriority_attr = if is_first_image {
                    r#" fetchpriority="high""#
                } else {
                    ""
                };

                let mut html = String::new();
                let _ = write!(
                    html,
                    r#"<figure class="image-container"><img src="{}" alt="{}" title="{}" loading="{}" decoding="async"{}{}{} /><figcaption>{}</figcaption></figure>"#,
                    escape_attr(&dest_url),
                    escape_attr(&alt_text),
                    escape_attr(&title),
                    loading_attr,
                    size_attrs,
                    srcset_attrs,
                    fetchpriority_attr,
                    caption_html
                );

                Some(Event::Html(CowStr::from(html)))
            }
            other => Some(other),
        }
    }
}

fn image_dimensions(dest_url: &str) -> Option<(u32, u32)> {
    // Only attempt for local files.
    if dest_url.starts_with("http://") || dest_url.starts_with("https://") {
        return None;
    }

    // Strip leading '/' to make it relative to project root.
    let cleaned = dest_url.trim_start_matches('/');
    let path = Path::new(cleaned);

    let path = if path.exists() {
        path.to_path_buf()
    } else {
        // Fall back to attempting the raw dest_url as given.
        Path::new(dest_url).to_path_buf()
    };

    imagesize::size(path)
        .ok()
        .map(|dim| (dim.width as u32, dim.height as u32))
}

impl<'a, I> Transformer<'a, I> for ImageCaptionTransformer<I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn transform(inner: I) -> Self {
        Self {
            inner,
            seen_first: false,
        }
    }
}

#[cfg(test)]
mod tests;
