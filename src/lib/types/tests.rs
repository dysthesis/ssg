use std::path::PathBuf;

use proptest::{
    prelude::*,
    string::string_regex,
    test_runner::{Config, TestRunner},
};

use super::{Href, IsoDate, RelPath, Tag};

prop_compose! {
    fn rel_components()(segments in proptest::collection::vec("[A-Za-z0-9]{1,10}", 1..4)) -> PathBuf {
        let mut p = PathBuf::new();
        for seg in segments {
            p.push(seg);
        }
        p
    }
}

#[test]
fn iso_date_roundtrips() {
    let mut runner = TestRunner::new(Config {
        cases: 32,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&(1970i32..=2100, 1u32..=12, 1u32..=28), |(year, month, day)| {
            let s = format!("{year:04}-{month:02}-{day:02}");
            let parsed = IsoDate::parse(&s).expect("valid date");
            prop_assert_eq!(parsed.as_str(), s);
            prop_assert_eq!(parsed.year(), year);
            Ok(())
        })
        .unwrap();
}

#[test]
fn iso_date_rejects_out_of_range() {
    let mut runner = TestRunner::new(Config {
        cases: 16,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&(1970i32..=2100, 13u32..=99, 32u32..=99), |(year, month, day)| {
            let s = format!("{year:04}-{month:02}-{day:02}");
            prop_assert!(IsoDate::parse(&s).is_none());
            Ok(())
        })
        .unwrap();
}

#[test]
fn tag_parse_accepts_valid() {
    let mut runner = TestRunner::new(Config {
        cases: 32,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&string_regex("[A-Za-z0-9_-]{1,16}").unwrap(), |s| {
            let tag = Tag::parse(&s).expect("should parse");
            prop_assert_eq!(tag.as_str(), s);
            Ok(())
        })
        .unwrap();
}

#[test]
fn tag_parse_rejects_invalid() {
    let mut runner = TestRunner::new(Config {
        cases: 32,
        failure_persistence: None,
        ..Config::default()
    });
    let bad_chars = prop_oneof![
        Just(" "), Just("!"), Just("@"), Just("#"), Just("$"), Just("%"), Just("^"), Just("&"),
        Just("*"), Just("+"), Just("="), Just("?"), Just(","), Just(";"), Just(":"), Just("/"), Just(".")
    ];
    runner
        .run(
            &(string_regex("[\\p{Alphabetic}\\p{Number}_-]{0,6}").unwrap(), bad_chars, string_regex("[\\p{Alphabetic}\\p{Number}_-]{0,6}").unwrap()),
            |(prefix, bad, suffix)| {
                let s = format!("{prefix}{bad}{suffix}");
                prop_assert!(Tag::parse(&s).is_none());
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn rel_path_accepts_relative() {
    let mut runner = TestRunner::new(Config {
        cases: 16,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&rel_components(), |p| {
            prop_assume!(!p.is_absolute());
            let rel = RelPath::new(p.clone()).expect("must accept relative");
            prop_assert_eq!(rel.as_path(), p.as_path());
            Ok(())
        })
        .unwrap();
}

#[test]
fn rel_path_rejects_absolute() {
    let abs = PathBuf::from("/tmp/abs/path");
    assert!(abs.is_absolute());
    assert!(RelPath::new(abs).is_none());
}

#[test]
fn href_uses_forward_slashes() {
    let mut runner = TestRunner::new(Config {
        cases: 16,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&rel_components(), |p| {
            let rel = RelPath::new(p.clone()).expect("relative");
            let href = Href::from_rel(&rel).as_str().to_string();
            prop_assert!(!href.contains('\\'));
            let expected = p.to_string_lossy().replace('\\', "/");
            prop_assert_eq!(href, expected);
            Ok(())
        })
        .unwrap();
}
