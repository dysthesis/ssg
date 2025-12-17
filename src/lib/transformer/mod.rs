use pulldown_cmark::Event;

pub mod code_block;

/// A transformer layer over an iterator of events, in order to allow custom
/// rendering strategies of different syntax elements
pub trait Transformer<'a>: Iterator<Item = Event<'a>> {}
