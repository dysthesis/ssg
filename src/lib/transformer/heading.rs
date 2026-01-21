use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};

use crate::transformer::Transformer;

/// Demote Markdown headings by one level:
/// h1 becomes h2, h2 becomes h3, and so on. h6 remains h6.
pub struct HeadingDemoterTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    inner: I,
}

impl<'a, I> Iterator for HeadingDemoterTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let ev = self.inner.next()?;
        Some(match ev {
            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            }) => Event::Start(Tag::Heading {
                level: demote(level),
                id,
                classes,
                attrs,
            }),

            Event::End(TagEnd::Heading(level)) => Event::End(TagEnd::Heading(demote(level))),

            other => other,
        })
    }
}

fn demote(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H2,
        HeadingLevel::H2 => HeadingLevel::H3,
        HeadingLevel::H3 => HeadingLevel::H4,
        HeadingLevel::H4 => HeadingLevel::H5,
        HeadingLevel::H5 => HeadingLevel::H6,
        HeadingLevel::H6 => HeadingLevel::H6,
    }
}

impl<'a, I> Transformer<'a, I> for HeadingDemoterTransformer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn transform(inner: I) -> Self {
        Self { inner }
    }
}
