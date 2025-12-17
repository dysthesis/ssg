use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};

use crate::renderer::CodeblockHighlighter;
use crate::transformer::Transformer;

pub enum CodeBlockState<'a> {
    /// Not in code block, pass through the event as-is.
    Passthrough,
    /// Currently inside a code block of language `lang`, so we accumulate all
    /// events until an `Event::End(TagEnd::CodeBlock)` is reached.
    Accumulating { lang: CodeBlockKind<'a> },
}

/// An adapter over pulldown_cmark parser in order to render code blocks with
/// custom strategies, e.g. tree-sitter-based highlighting using `syntect`
pub struct CodeBlockTransformer<'a, I, H>
where
    I: Iterator<Item = Event<'a>>,
    H: CodeblockHighlighter,
{
    /// The inner iterator. Can be the raw `Parser`, another `Transformer`, or
    /// other iterators over `Event<'a>`.
    inner: I,
    /// The highlighting strategy to use.
    highlighter: &'a H,
    /// Buffer to accumulate any code.
    buffer: String,
    /// Current state of the transformer; are we inside a code block?
    state: CodeBlockState<'a>, // events are bound to the Markdown source
                               // string; likewise is the language name for the
                               // current state
}

impl<'a, I, H> Iterator for CodeBlockTransformer<'a, I, H>
where
    I: Iterator<Item = Event<'a>>,
    H: CodeblockHighlighter,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let event = self.inner.next()?;
            match &self.state {
                CodeBlockState::Passthrough => match event {
                    Event::Start(Tag::CodeBlock(lang)) => {
                        // Transition to accumulating state
                        self.state = CodeBlockState::Accumulating { lang };
                        self.buffer.clear();
                        // Don't return anything—swallow the Start event
                        continue;
                    }
                    // All other events pass through unchanged
                    other => return Some(other),
                },
                CodeBlockState::Accumulating { lang: _ } => {
                    match event {
                        Event::Text(text) => {
                            // Accumulate text into our buffer
                            self.buffer.push_str(&text);
                            // Swallow this event too
                            continue;
                        }
                        Event::End(TagEnd::CodeBlock) => {
                            // Extract the kind before transitioning state
                            let CodeBlockState::Accumulating { lang } =
                                std::mem::replace(&mut self.state, CodeBlockState::Passthrough)
                            else {
                                unreachable!()
                            };

                            // Convert CodeBlockKind to Option<&str>
                            let language = match lang {
                                CodeBlockKind::Fenced(ref l) => Some(l.as_ref()),
                                CodeBlockKind::Indented => None,
                            };

                            // Perform the actual highlighting
                            let html = self.highlighter.render_codeblock(&self.buffer, language);

                            // Return the transformed content as an Html event
                            return Some(Event::Html(html.into_cow_str()));
                        }
                        // Unexpected event inside code block. It's probably a
                        // malformed input
                        other => {
                            tracing::warn!("unexpected event inside code block: {:?}", other);
                            continue;
                        }
                    }
                }
            }
        }
    }
}

impl<'a, I, H> Transformer<'a> for CodeBlockTransformer<'a, I, H>
where
    I: Iterator<Item = Event<'a>>,
    H: CodeblockHighlighter,
{
}

pub trait ToCodeBlockTransformer<'a>: Iterator<Item = Event<'a>> + Sized {
    fn highlight_code<H>(self, highlighter: &'a H) -> CodeBlockTransformer<'a, Self, H>
    where
        H: CodeblockHighlighter,
    {
        CodeBlockTransformer {
            inner: self,
            buffer: String::new(),
            state: CodeBlockState::Passthrough,
            highlighter,
        }
    }
}

impl<'a, I> ToCodeBlockTransformer<'a> for I where I: Iterator<Item = Event<'a>> {}
