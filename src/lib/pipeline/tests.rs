use std::{
    fs,
    path::{Path, PathBuf},
};

use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::{
    config::{INPUT_DIR, OUTPUT_DIR, POSTS_DIR, TAGS_DIR},
    pipeline::{build_at, build_once},
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

    build_once().unwrap();

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
