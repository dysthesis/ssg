use std::collections::{HashMap, HashSet};

use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
use pulldown_cmark::{CowStr, Event, HeadingLevel, Tag, TagEnd};

use crate::{transformer::toc::insert_toc_and_heading_ids, utils::slugify};

#[test]
fn toc_assigns_unique_ids() {
    let mut runner = TestRunner::new(Config {
        cases: 16,
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(
            &proptest::collection::vec("[A-Za-z0-9 ]{1,16}", 1..6),
            |headings| {
                let mut events = Vec::new();
                for title in &headings {
                    events.push(Event::Start(Tag::Heading { level: HeadingLevel::H2, id: None, classes: vec![], attrs: vec![] }));
                    events.push(Event::Text(CowStr::from(title.clone())));
                    events.push(Event::End(TagEnd::Heading(HeadingLevel::H2)));
                }

                let out = insert_toc_and_heading_ids(events);

                let mut ids = Vec::new();
                for ev in &out {
                    if let Event::Start(Tag::Heading { level: HeadingLevel::H2, id: Some(id), .. }) = ev {
                        ids.push(id.to_string());
                    }
                }

                prop_assert_eq!(ids.len(), headings.len());
                let mut seen_ids = HashSet::new();
                let mut counts: HashMap<String, usize> = HashMap::new();
                for (title, id) in headings.iter().zip(ids.iter()) {
                    prop_assert!(seen_ids.insert(id.clone()), "duplicate id {}", id);

                    let slug = slugify(title);
                    let entry = counts.entry(slug.clone()).or_insert(0);
                    *entry += 1;
                    let expected = if *entry == 1 { slug.clone() } else { format!("{slug}-{}", *entry) };
                    prop_assert_eq!(id, &expected);
                }

                prop_assert!(matches!(out.first(), Some(Event::Html(_))));
                Ok(())
            },
        )
        .unwrap();
}
