use proptest::{prelude::*, test_runner::{Config, TestRunner}};
use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};

use crate::transformer::{heading::HeadingDemoterTransformer, WithTransformer};

#[test]
fn heading_demoter_increments_level() {
    let mut runner = TestRunner::new(Config {
        cases: 32,
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(
            &prop_oneof![
                Just(HeadingLevel::H1),
                Just(HeadingLevel::H2),
                Just(HeadingLevel::H3),
                Just(HeadingLevel::H4),
                Just(HeadingLevel::H5),
                Just(HeadingLevel::H6),
            ],
            |level| {
                let events = vec![
                    Event::Start(Tag::Heading { level, id: None, classes: vec![], attrs: vec![] }),
                    Event::End(TagEnd::Heading(level)),
                ];
                let out: Vec<_> = events.into_iter().with_transformer::<HeadingDemoterTransformer<_>>().collect();

                match (&out[0], &out[1]) {
                    (
                        Event::Start(Tag::Heading { level: out_start, .. }),
                        Event::End(TagEnd::Heading(out_end)),
                    ) => {
                        let expected = match level {
                            HeadingLevel::H1 => HeadingLevel::H2,
                            HeadingLevel::H2 => HeadingLevel::H3,
                            HeadingLevel::H3 => HeadingLevel::H4,
                            HeadingLevel::H4 => HeadingLevel::H5,
                            HeadingLevel::H5 => HeadingLevel::H6,
                            HeadingLevel::H6 => HeadingLevel::H6,
                        };
                        prop_assert_eq!(*out_start, expected);
                        prop_assert_eq!(*out_end, expected);
                    }
                    _ => prop_assert!(false, "unexpected events"),
                }
                Ok(())
            },
        )
        .unwrap();
}
