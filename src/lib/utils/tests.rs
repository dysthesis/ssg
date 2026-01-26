use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};

use super::{escape_text, prefix_to_root, slugify};

#[test]
fn escape_text_removes_angle_and_quotes() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&".*", |s| {
            let escaped = escape_text(&s);
            for ch in ['<', '>', '"', '\''] {
                prop_assert!(!escaped.contains(ch));
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn escape_text_noops_when_safe() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&"[^<>'\"&]*", |s| {
            let escaped = escape_text(&s);
            prop_assert_eq!(escaped, s);
            Ok(())
        })
        .unwrap();
}

#[test]
fn slugify_constrains_charset() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&".*", |input| {
            let slug = slugify(&input);
            prop_assert!(!slug.is_empty());
            prop_assert!(!slug.contains(char::is_whitespace));
            Ok(())
        })
        .unwrap();
}

#[test]
fn prefix_to_root_matches_depth() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &proptest::collection::vec("[A-Za-z0-9]{1,10}", 0..4),
            |segments| {
                use std::path::PathBuf;
                let mut rel = PathBuf::new();
                for seg in segments {
                    rel.push(seg);
                }
                rel.set_extension("md");
                let depth = rel.parent().map(|pp| pp.components().count()).unwrap_or(0);
                let expected = "../".repeat(depth);
                prop_assert_eq!(prefix_to_root(&rel), expected);
                Ok(())
            },
        )
        .unwrap();
}
