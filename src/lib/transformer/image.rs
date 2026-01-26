use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use std::fmt::Write;

use crate::{transformer::Transformer, utils::escape_attr};

pub struct ImageCaptionTransformer<I> {
    inner: I,
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
                // We found an image. We need to consume events until the end of the image
                // to capture the "Alt text" / "Caption".
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
                        None => break, // Should not happen in valid markdown
                    }
                }

                // 1. Render inner events to HTML for the <figcaption> (supports formatting like *italics*)
                let mut caption_html = String::new();
                pulldown_cmark::html::push_html(&mut caption_html, alt_events.iter().cloned());

                // 2. Extract plain text for the <img> alt attribute (no HTML allowed here)
                let mut alt_text = String::new();
                for e in &alt_events {
                    match e {
                        Event::Text(t) | Event::Code(t) => alt_text.push_str(t),
                        _ => {}
                    }
                }

                // 3. Build the <figure> structure
                let mut html = String::new();
                let _ = write!(
                    html,
                    r#"<figure class="image-container"><img src="{}" alt="{}" title="{}" /><figcaption>{}</figcaption></figure>"#,
                    escape_attr(&dest_url),
                    escape_attr(&alt_text),
                    escape_attr(&title),
                    caption_html
                );

                Some(Event::Html(CowStr::from(html)))
            }
            other => Some(other),
        }
    }
}

impl<'a, I> Transformer<'a, I> for ImageCaptionTransformer<I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn transform(inner: I) -> Self {
        Self { inner }
    }
}
