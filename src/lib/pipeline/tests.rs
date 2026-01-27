use std::{
    fs,
    path::{Path, PathBuf},
};

use atom_syndication;
use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
use rss;
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::{
    config::{INPUT_DIR, OUTPUT_DIR, POSTS_DIR, SITE_BASE_URL, SITE_DEFAULT_OG_IMAGE, TAGS_DIR},
    pipeline::build_at,
};

// Simple guard to restore cwd even on panic.
struct DirGuard(std::path::PathBuf);
impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

prop_compose! {
fn rel_markdown_path()(segments in proptest::collection::vec("[A-Za-z0-9]{1,10}", 1..4)) -> PathBuf {
    let mut p = PathBuf::new();
    for seg in segments {
        p.push(seg);
    }
    p.set_extension("md");
    p
}
}

fn write_md(root: &Path, rel_path: &Path, body: &str) -> std::io::Result<()> {
    let full = root.join(INPUT_DIR).join(rel_path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, body)
}

fn snapshot_public(root: &Path) -> std::io::Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(root).unwrap().to_path_buf();
            out.push((rel, fs::read(entry.path())?));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn public_path(tmp: &TempDir, rel: impl AsRef<Path>) -> PathBuf {
    tmp.path().join(OUTPUT_DIR).join(rel.as_ref())
}

fn read_public(tmp: &TempDir, rel: impl AsRef<Path>) -> String {
    fs::read_to_string(public_path(tmp, rel)).expect("public file")
}

fn read_public_bytes(tmp: &TempDir, rel: impl AsRef<Path>) -> Vec<u8> {
    fs::read(public_path(tmp, rel)).expect("public file bytes")
}

#[test]
fn build_once_emits_expected_paths() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(
            &(rel_markdown_path(), any::<bool>()),
            |(rel_path, has_math)| {
                let tmp = TempDir::new().expect("tempdir");

                let content_dir = PathBuf::from(INPUT_DIR);
                let full_md = tmp.path().join(&content_dir).join(&rel_path);
                std::fs::create_dir_all(full_md.parent().unwrap()).unwrap();

                let math = if has_math {
                    "This has $x^2$ math."
                } else {
                    "Plain text."
                };
                let md =
                    format!("---\ntitle: Example\nctime: 2024-01-01\n---\n# Heading\n{math}\n");
                std::fs::write(&full_md, md).unwrap();

                std::fs::write(tmp.path().join("style.css"), "body { color: black; }").unwrap();

                build_at(tmp.path()).unwrap();

                let rel_out = PathBuf::from(POSTS_DIR).join(rel_path.with_extension("html"));
                let out_file = tmp.path().join(OUTPUT_DIR).join(&rel_out);
                prop_assert!(out_file.exists());

                prop_assert!(tmp.path().join(OUTPUT_DIR).join("index.html").exists());

                let html = std::fs::read_to_string(&out_file).unwrap();
                let depth = rel_out
                    .parent()
                    .map(|p| p.components().count())
                    .unwrap_or(0);
                let expected_prefix = "../".repeat(depth);
                let expected_piece = format!("{}style.css", expected_prefix);
                prop_assert!(html.contains(&expected_piece));

                if has_math {
                    prop_assert!(html.contains("katex.min.css"));
                } else {
                    prop_assert!(!html.contains("katex.min.css"));
                }
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn build_is_deterministic_across_runs() {
    let tmp = TempDir::new().expect("tempdir");
    let cwd = std::env::current_dir().unwrap();
    let _guard = DirGuard(cwd);
    std::env::set_current_dir(tmp.path()).unwrap();

    fs::create_dir_all(INPUT_DIR).unwrap();
    fs::write("style.css", "body { color: black; }").unwrap();

    let md = "---\ntitle: Deterministic\nctime: 2024-02-02\n---\nHello world.\n";
    write_md(tmp.path(), Path::new("single.md"), md).unwrap();

    build_at(tmp.path()).unwrap();
    let first = snapshot_public(&tmp.path().join(OUTPUT_DIR)).unwrap();

    build_at(tmp.path()).unwrap();
    let second = snapshot_public(&tmp.path().join(OUTPUT_DIR)).unwrap();

    assert_eq!(first, second);
}

#[test]
fn math_pages_toggle_katex_link() {
    let tmp = TempDir::new().expect("tempdir");

    fs::create_dir_all(INPUT_DIR).unwrap();
    fs::write("style.css", "body { color: black; }").unwrap();

    let math = "---\ntitle: Mathy\nctime: 2024-03-01\n---\nInline $x^2$ and $$y^2$$.\n";
    let plain = "---\ntitle: Plain\nctime: 2024-03-02\n---\nNo math here.\n";
    write_md(tmp.path(), Path::new("math.md"), math).unwrap();
    write_md(tmp.path(), Path::new("plain.md"), plain).unwrap();

    build_at(tmp.path()).unwrap();

    let math_html = read_public(&tmp, Path::new(POSTS_DIR).join("math.html"));
    let plain_html = read_public(&tmp, Path::new(POSTS_DIR).join("plain.html"));

    assert!(math_html.contains("katex.min.css"));
    assert!(!plain_html.contains("katex.min.css"));
}

#[test]
fn tag_pages_are_filtered_and_sorted() {
    let tmp = TempDir::new().expect("tempdir");

    fs::create_dir_all(INPUT_DIR).unwrap();
    fs::write("style.css", "body { color: black; }").unwrap();

    let docs = vec![
        ("newer", "2025-05-05", "rust"),
        ("older", "2024-01-01", "rust"),
        ("other", "2023-01-01", "life"),
        ("badtag", "2024-06-06", "bad tag"),
    ];

    for (title, date, tag) in &docs {
        let md = format!("---\ntitle: {title}\nctime: {date}\ntags: [{tag}]\n---\nBody\n");
        write_md(tmp.path(), Path::new(&format!("{title}.md")), &md).unwrap();
    }

    build_at(tmp.path()).unwrap();

    let rust_path = Path::new(TAGS_DIR).join("rust.html");
    let rust_html = read_public(&tmp, rust_path);
    assert!(rust_html.contains("newer"));
    assert!(rust_html.contains("older"));
    assert!(!rust_html.contains("other"));

    let pos_new = rust_html.find("newer").unwrap();
    let pos_old = rust_html.find("older").unwrap();
    assert!(
        pos_new < pos_old,
        "rust tag page must be sorted by date desc then title"
    );

    let bad_tag_path = tmp
        .path()
        .join(OUTPUT_DIR)
        .join(TAGS_DIR)
        .join("bad tag.html");
    assert!(!bad_tag_path.exists(), "invalid tags should be discarded");
}

#[test]
fn asset_prefixes_match_depth() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(&rel_markdown_path(), |rel_path| {
            let tmp = TempDir::new().expect("tempdir");

            fs::create_dir_all(INPUT_DIR).unwrap();
            fs::write("style.css", "body { color: black; }").unwrap();

            let md = "---\ntitle: PrefixTest\nctime: 2024-04-04\n---\nContent\n";
            write_md(tmp.path(), rel_path.as_path(), md).unwrap();

            build_at(tmp.path()).unwrap();

            let rel_out = PathBuf::from(POSTS_DIR).join(rel_path.with_extension("html"));
            let html = read_public(&tmp, rel_out.clone());

            let depth = rel_out
                .parent()
                .map(|p| p.components().count())
                .unwrap_or(0);
            let prefix = "../".repeat(depth);

            fn has_href(html: &str, target: &str) -> bool {
                html.contains(&format!(r#"href="{}""#, target))
                    || html.contains(&format!("href={}", target))
            }

            let css_target = format!("{prefix}style.css");
            let index_target = format!("{prefix}index.html");

            prop_assert!(has_href(&html, &css_target));
            prop_assert!(has_href(&html, &index_target));
            Ok(())
        })
        .unwrap();
}

#[test]
fn feeds_are_emitted_and_sorted_with_absolute_links() {
    let tmp = TempDir::new().expect("tempdir");

    fs::create_dir_all(INPUT_DIR).unwrap();
    fs::write("style.css", "body { color: black; }").unwrap();

    // Older post
    let older = "---\ntitle: Older\nctime: 2024-01-01\n---\nBody\n";
    write_md(tmp.path(), Path::new("older.md"), older).unwrap();

    // Newer post with tag
    let newer = "---\ntitle: Newer\nctime: 2025-01-01\nmtime: 2025-01-02\ntags: [rust]\ndescription: Summary here\n---\nBody\n";
    write_md(tmp.path(), Path::new("newer.md"), newer).unwrap();

    build_at(tmp.path()).unwrap();

    // RSS assertions
    let rss_bytes = read_public_bytes(&tmp, Path::new("rss.xml"));
    let channel = rss::Channel::read_from(&rss_bytes[..]).expect("parse rss");
    assert_eq!(channel.items().len(), 2);
    assert_eq!(channel.items()[0].title(), Some("Newer"));
    assert!(
        channel.items()[0]
            .link()
            .unwrap()
            .starts_with(SITE_BASE_URL.trim_end_matches('/'))
    );
    assert_eq!(channel.items()[0].description(), Some("Summary here"));
    let content = channel.items()[0].content().expect("rss content");
    assert!(
        content.contains("<p>Body</p>"),
        "RSS content should include full body HTML"
    );
    assert!(
        content.contains("<h1>Newer</h1>"),
        "RSS content should include the article header"
    );
    let categories: Vec<_> = channel.items()[0]
        .categories()
        .iter()
        .map(|c| c.name())
        .collect();
    assert!(categories.contains(&"rust"));

    // Atom assertions
    let atom_bytes = read_public_bytes(&tmp, Path::new("atom.xml"));
    let feed = atom_syndication::Feed::read_from(&atom_bytes[..]).expect("parse atom");
    assert_eq!(feed.entries().len(), 2);
    assert_eq!(feed.entries()[0].title().to_string(), "Newer");
    assert!(
        feed.entries()[0]
            .links()
            .first()
            .unwrap()
            .href()
            .starts_with(SITE_BASE_URL.trim_end_matches('/'))
    );
    assert_eq!(
        feed.entries()[0].summary().map(|s| s.as_str()),
        Some("Summary here")
    );
    let atom_content = feed.entries()[0]
        .content()
        .and_then(|c| c.value())
        .expect("atom content");
    assert!(
        atom_content.contains("<p>Body</p>"),
        "Atom content should include full body HTML"
    );
    assert!(
        atom_content.contains("<h1>Newer</h1>"),
        "Atom content should include the article header"
    );
    let atom_cats: Vec<_> = feed.entries()[0]
        .categories()
        .iter()
        .map(|c| c.term())
        .collect();
    assert!(atom_cats.contains(&"rust"));
}

#[test]
fn feeds_render_plain_footnotes() {
    let tmp = TempDir::new().expect("tempdir");

    fs::create_dir_all(INPUT_DIR).unwrap();
    fs::write("style.css", "body { color: black; }").unwrap();

    let md = r#"---
title: Footy
ctime: 2025-04-04
---
Body with footnote[^1].

[^1]: This is the footnote, rendered plainly.
"#;
    write_md(tmp.path(), Path::new("note.md"), md).unwrap();

    build_at(tmp.path()).unwrap();

    let rss_bytes = read_public_bytes(&tmp, Path::new("rss.xml"));
    let channel = rss::Channel::read_from(&rss_bytes[..]).expect("parse rss");
    let content = channel.items()[0].content().expect("rss content");

    assert!(content.contains(r#"<sup id="fnref-1""#));
    assert!(content.contains(r#"<section class="footnotes""#));
    assert!(content.contains("This is the footnote"));
    assert!(!content.contains("margin-toggle"));
    assert!(!content.contains("sidenote"));
}

#[test]
fn article_pages_include_opengraph_meta_with_absolute_urls() {
    let tmp = TempDir::new().expect("tempdir");

    fs::create_dir_all(INPUT_DIR).unwrap();
    fs::write("style.css", "body { color: black; }").unwrap();

    let md = r#"---
title: OG Title
description: Short desc
ctime: 2025-01-01
image: images/pic.png
---
Body
"#;
    write_md(tmp.path(), Path::new("post.md"), md).unwrap();

    build_at(tmp.path()).unwrap();

    let html = read_public(&tmp, Path::new(POSTS_DIR).join("post.html"));
    let base = SITE_BASE_URL.trim_end_matches('/');

    assert!(html.contains("property=og:title"));
    assert!(html.contains("OG Title"));
    assert!(html.contains("property=og:description"));
    assert!(html.contains("Short desc"));
    assert!(html.contains("property=og:type"));
    assert!(html.contains("article"));
    assert!(html.contains("property=og:url"));
    assert!(html.contains(&format!("{base}/posts/post.html")));
    assert!(html.contains("property=og:image"));
    assert!(html.contains(&format!("{base}/images/pic.png")));
    assert!(html.contains("rel=canonical"));
}

#[test]
fn default_social_image_is_used_when_frontmatter_is_absent() {
    let tmp = TempDir::new().expect("tempdir");

    fs::create_dir_all(INPUT_DIR).unwrap();
    fs::write("style.css", "body { color: black; }").unwrap();

    let md = r#"---
title: No Image
description: Uses default
ctime: 2025-02-01
---
Body
"#;
    write_md(tmp.path(), Path::new("no-image.md"), md).unwrap();

    build_at(tmp.path()).unwrap();

    let html = read_public(&tmp, Path::new(POSTS_DIR).join("no-image.html"));
    let base = SITE_BASE_URL.trim_end_matches('/');

    if let Some(default_img) = SITE_DEFAULT_OG_IMAGE {
        assert!(html.contains("property=og:image"));
        assert!(html.contains(&format!("{}/{}", base, default_img.trim_start_matches('/'))));
    } else {
        panic!("SITE_DEFAULT_OG_IMAGE must be set for this test");
    }
}

#[test]
fn index_page_includes_generic_og_meta() {
    let tmp = TempDir::new().expect("tempdir");

    fs::create_dir_all(INPUT_DIR).unwrap();
    fs::write("style.css", "body { color: black; }").unwrap();

    let md = r#"---
title: Any
ctime: 2025-03-03
---
Body
"#;
    write_md(tmp.path(), Path::new("any.md"), md).unwrap();

    build_at(tmp.path()).unwrap();

    let html = read_public(&tmp, Path::new("index.html"));
    let base = SITE_BASE_URL.trim_end_matches('/');

    assert!(html.contains("property=og:type"));
    assert!(html.contains("website"));
    assert!(html.contains("property=og:url"));
    assert!(html.contains(&format!("{base}/index.html")));
    assert!(html.contains("Index"));
}
