use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use tempfile::TempDir;

use ssg::config::INPUT_DIR;

/// Options to synthesize a markdown site for benchmarking.
#[derive(Clone, Debug)]
pub struct SiteOptions {
    pub posts: usize,
    pub body_bytes: usize,
    pub with_code: bool,
    pub with_math: bool,
    pub with_footnotes: bool,
    pub with_images: bool,
}

impl Default for SiteOptions {
    fn default() -> Self {
        Self {
            posts: 10,
            body_bytes: 2_000,
            with_code: true,
            with_math: true,
            with_footnotes: true,
            with_images: false,
        }
    }
}

/// Generate a temporary site tree under a fresh TempDir.
/// The returned TempDir keeps the files alive for the caller's lifetime.
pub fn make_site(opts: &SiteOptions) -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();

    // Minimal stylesheet required by the pipeline.
    fs::write(root.join("style.css"), "body { color: black; }").expect("write style");

    if opts.with_images {
        fs::create_dir_all(root.join("assets")).expect("assets dir");
    }

    let body_chunk = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ";
    let chunk_repeat = (opts.body_bytes / body_chunk.len()).max(1);
    let body_base: String = body_chunk.repeat(chunk_repeat);

    for i in 0..opts.posts {
        let mut body = body_base.clone();

        if opts.with_code {
            body.push_str("\n\n```rs\nfn main() { println!(\"hi\"); }\n```\n");
        }
        if opts.with_math {
            body.push_str("\n\nThis has inline math $x^2$ and display:\n\n$$ x^2 + y^2 $$\n");
        }
        if opts.with_footnotes {
            body.push_str("\nFootnote ref[^1].\n\n[^1]: side note\n");
        }
        if opts.with_images {
            let img_name = format!("assets/pic{i}.png");
            body.push_str(&format!("\n![alt text]({img_name})\n"));
            write_tiny_png(root.join(&img_name));
        }

        let title = format!("Post {i:04}");
        let date = format!("2025-{:02}-{:02}", (i % 12) + 1, (i % 28) + 1);
        let markdown = format!(
            r#"---
title: {title}
ctime: {date}
tags: [bench]
---

# Heading One

{body}
"#
        );

        let rel = PathBuf::from(INPUT_DIR).join(format!("post-{i}.md"));
        let full = root.join(&rel);
        fs::create_dir_all(full.parent().unwrap()).expect("contents dir");
        fs::write(full, markdown).expect("write markdown");
    }

    tmp
}

/// A tiny 1x1 transparent PNG used for image dimension probing.
fn write_tiny_png(path: impl AsRef<Path>) {
    // This is a minimal PNG for a 1x1 transparent pixel.
    const PNG_BYTES: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    if let Some(parent) = path.as_ref().parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, PNG_BYTES);
}

/// Build an `Event` stream representing a fenced code block.
pub fn code_block_events(code: &str) -> Vec<Event<'static>> {
    vec![
        Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(
            CowStr::from("rs"),
        ))),
        Event::Text(CowStr::from(code.to_string())),
        Event::End(TagEnd::CodeBlock),
    ]
}

/// Build an `Event` stream containing many headings for TOC tests.
pub fn heading_events(count_h2: usize, count_h3: usize) -> Vec<Event<'static>> {
    let mut events = Vec::new();
    for i in 0..count_h2 {
        events.push(Event::Start(Tag::Heading {
            level: pulldown_cmark::HeadingLevel::H2,
            id: None,
            classes: vec![],
            attrs: vec![],
        }));
        events.push(Event::Text(CowStr::from(format!("Section {i}"))));
        events.push(Event::End(TagEnd::Heading(
            pulldown_cmark::HeadingLevel::H2,
        )));
        for j in 0..count_h3 {
            events.push(Event::Start(Tag::Heading {
                level: pulldown_cmark::HeadingLevel::H3,
                id: None,
                classes: vec![],
                attrs: vec![],
            }));
            events.push(Event::Text(CowStr::from(format!("Sub {i}-{j}"))));
            events.push(Event::End(TagEnd::Heading(
                pulldown_cmark::HeadingLevel::H3,
            )));
        }
    }
    events
}

/// Footnote reference + definition pair.
pub fn footnote_events(def_len: usize) -> Vec<Event<'static>> {
    let def_body = "note ".repeat(def_len.max(1));
    vec![
        Event::FootnoteReference(CowStr::from("a")),
        Event::Start(Tag::FootnoteDefinition(CowStr::from("a"))),
        Event::Start(Tag::Paragraph),
        Event::Text(CowStr::from(def_body)),
        Event::End(TagEnd::Paragraph),
        Event::End(TagEnd::FootnoteDefinition),
    ]
}

/// Inline + display math events.
pub fn math_events(expr: &str) -> Vec<Event<'static>> {
    vec![
        Event::InlineMath(CowStr::from(expr.to_string())),
        Event::DisplayMath(CowStr::from(expr.to_string())),
    ]
}

/// Build a small Rust snippet of the requested number of lines.
pub fn rust_snippet(lines: usize) -> String {
    let mut s = String::new();
    for i in 0..lines {
        s.push_str(&format!("fn f{i}() {{ println!(\"{i}\"); }}\n"));
    }
    s
}

/// Convenience to express durations in seconds without importing chrono.
pub fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}
