//! A transformer is an adapter over an iterator of pulldown-cmark `Event`. It
//! intercepts any incoming event(s) that are of interest, and outputs a
//! transformed event. For example, a code block transformer may choose to
//! consume a sequence of events from some `Event::Start(Tag::CodeBlock(lang))`
//! to Event::End(TagEnd::CodeBlock) and return `Event::Html(html.into_cow_str())`
//! in order to perform things such as syntax highlighting.
use pulldown_cmark::Event;

pub mod code_block;
pub mod footnote;
pub mod heading;
pub mod math;

/// A transformer over events, that takes in an inner iterator and returns
/// another iterator of events, which returns transformed events.
pub trait Transformer<'a, I>: Iterator<Item = Event<'a>> + Sized
where
    I: Iterator<Item = Event<'a>>,
{
    /// Wrap an inner iterator with the transformer
    fn transform(inner: I) -> Self;
}

/// Wrap an event iterator with another transformer, allowing for chaining.
pub trait WithTransformer<'a>: Iterator<Item = Event<'a>> + Sized {
    /// Wrap ourselves with some transformer
    fn with_transformer<T: Transformer<'a, Self>>(self) -> T {
        T::transform(self)
    }
}

/// Blanket implementation over any event iterator
impl<'a, I: Iterator<Item = Event<'a>>> WithTransformer<'a> for I {}
