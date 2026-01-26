use std::path::PathBuf;

use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};

use crate::{
    article::Article,
    types::{Href, IsoDate, RelPath},
};

#[test]
fn listing_groups_by_year() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(
            &proptest::collection::vec(("[A-Za-z0-9]{3,8}", 1990i32..=2025), 1..5),
            |items| {
                let mut articles = Vec::new();
                for (title, year) in items.iter() {
                    let date = IsoDate::parse(&format!("{year:04}-01-01")).unwrap();
                    articles.push(Article {
                        title: title.clone(),
                        ctime: Some(date),
                        updated: None,
                        summary: None,
                        href: Href::from_rel(
                            &RelPath::new(PathBuf::from(format!("{title}.html"))).unwrap(),
                        ),
                        tags: vec![],
                    });
                }
                articles.sort_by(|a, b| b.ctime.cmp(&a.ctime));
                let body =
                    crate::article::render_listing_page("Page", "Heading", &articles, "", "");
                for a in &articles {
                    let year_str = a.ctime.as_ref().unwrap().year().to_string();
                    prop_assert!(body.contains(&year_str));
                    prop_assert!(body.contains(&a.title));
                }
                Ok(())
            },
        )
        .unwrap();
}
