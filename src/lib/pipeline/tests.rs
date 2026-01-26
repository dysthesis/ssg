use std::path::PathBuf;

use proptest::{prelude::*, test_runner::{Config, TestRunner}};
use tempfile::TempDir;

use crate::{
    config::{INPUT_DIR, OUTPUT_DIR, POSTS_DIR},
    pipeline::build_once,
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

#[test]
fn build_once_emits_expected_paths() {
    let mut runner = TestRunner::new(Config {
        cases: 2,
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(&(rel_markdown_path(), any::<bool>()), |(rel_path, has_math)| {
            let tmp = TempDir::new().expect("tempdir");
            let cwd = std::env::current_dir().unwrap();
            let _guard = DirGuard(cwd.clone());
            std::env::set_current_dir(tmp.path()).unwrap();

            let content_dir = PathBuf::from(INPUT_DIR);
            let full_md = tmp.path().join(&content_dir).join(&rel_path);
            std::fs::create_dir_all(full_md.parent().unwrap()).unwrap();

            let math = if has_math { "This has $x^2$ math." } else { "Plain text." };
            let md = format!(
                "---\ntitle: Example\nctime: 2024-01-01\n---\n# Heading\n{math}\n"
            );
            std::fs::write(&full_md, md).unwrap();

            std::fs::write(tmp.path().join("style.css"), "body { color: black; }").unwrap();

            build_once().unwrap();

            let rel_out = PathBuf::from(POSTS_DIR).join(rel_path.with_extension("html"));
            let out_file = tmp.path().join(OUTPUT_DIR).join(&rel_out);
            prop_assert!(out_file.exists());

            prop_assert!(tmp.path().join(OUTPUT_DIR).join("index.html").exists());

            let html = std::fs::read_to_string(&out_file).unwrap();
            let depth = rel_out.parent().map(|p| p.components().count()).unwrap_or(0);
            let expected_prefix = "../".repeat(depth);
            let expected_piece = format!("{}style.css", expected_prefix);
            prop_assert!(html.contains(&expected_piece));

            if has_math {
                prop_assert!(html.contains("katex.min.css"));
            } else {
                prop_assert!(!html.contains("katex.min.css"));
            }
            Ok(())
        })
        .unwrap();
}
