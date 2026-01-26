use dhat::{DhatAlloc, Profiler};
use pulldown_cmark::Event;

use ssg::transformer::{
    WithTransformer, code_block::CodeHighlightTransformer, footnote::FootnoteTransformer,
    image::ImageCaptionTransformer, math::MathTransformer, toc::TocTransformer,
};

mod fixtures;
use fixtures::{code_block_events, footnote_events, heading_events, math_events, rust_snippet};

#[global_allocator]
static ALLOC: DhatAlloc = DhatAlloc;

fn main() {
    let _prof = Profiler::builder()
        .file_name("dhat-transformers.json")
        .build();

    // Build a large synthetic event stream that exercises every transformer.
    let mut events: Vec<Event<'static>> = Vec::new();
    events.extend(code_block_events(&rust_snippet(2_000)));
    events.extend(math_events("a^2 + b^2 = c^2"));
    events.extend(footnote_events(80));
    events.extend(heading_events(120, 2));

    // Add an image to flow through ImageCaptionTransformer.
    events.push(pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image {
        link_type: pulldown_cmark::LinkType::Inline,
        dest_url: pulldown_cmark::CowStr::from("https://example.com/p.png"),
        title: pulldown_cmark::CowStr::from("pic"),
        id: pulldown_cmark::CowStr::from(""),
    }));
    events.push(pulldown_cmark::Event::Text(pulldown_cmark::CowStr::from(
        "caption",
    )));
    events.push(pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Image));

    let out: Vec<_> = events
        .into_iter()
        .with_transformer::<CodeHighlightTransformer<_>>()
        .with_transformer::<MathTransformer<_>>()
        .with_transformer::<FootnoteTransformer<_>>()
        .with_transformer::<TocTransformer<'_, _>>()
        .with_transformer::<ImageCaptionTransformer<_>>()
        .collect();

    // Ensure the transformed events stay alive until after the profile.
    dhat::md::black_box(out.len());
}
