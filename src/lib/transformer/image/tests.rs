use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};

use crate::transformer::{WithTransformer, image::ImageCaptionTransformer};
use crate::utils::escape_attr;

#[test]
fn image_caption_wraps_in_html() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(
            &("https?://[A-Za-z0-9./_-]{1,24}", ".*", ".{1,20}"),
            |(dest, title, alt)| {
                let events = vec![
                    Event::Start(Tag::Image {
                        link_type: LinkType::Inline,
                        dest_url: CowStr::from(dest.clone()),
                        title: CowStr::from(title.clone()),
                        id: CowStr::from(""),
                    }),
                    Event::Text(CowStr::from(alt.clone())),
                    Event::End(TagEnd::Image),
                ];

                let out: Vec<_> = events
                    .into_iter()
                    .with_transformer::<ImageCaptionTransformer<_>>()
                    .collect();
                prop_assert_eq!(out.len(), 1);
                match &out[0] {
                    Event::Html(html) => {
                        let s = html.to_string();
                        let expected_src = format!(r#"src="{}""#, escape_attr(&dest));
                        let expected_alt = format!(r#"alt="{}""#, escape_attr(&alt));
                        prop_assert!(s.contains(r#"<figure class="image-container">"#));
                        prop_assert!(s.contains(&expected_src));
                        prop_assert!(s.contains(&expected_alt));
                        prop_assert!(s.contains("<figcaption>"));
                        prop_assert!(s.contains(r#"loading="eager""#));
                        prop_assert!(s.contains(r#"decoding="async""#));
                        prop_assert!(s.contains(r#"fetchpriority="high""#));
                    }
                    _ => prop_assert!(false, "expected Html event"),
                }
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn second_image_is_lazy_and_not_high_priority() {
    let events = vec![
        Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url: CowStr::from("foo.jpg"),
            title: CowStr::from(""),
            id: CowStr::from(""),
        }),
        Event::Text(CowStr::from("first")),
        Event::End(TagEnd::Image),
        Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url: CowStr::from("bar.jpg"),
            title: CowStr::from(""),
            id: CowStr::from(""),
        }),
        Event::Text(CowStr::from("second")),
        Event::End(TagEnd::Image),
    ];

    let out: Vec<_> = events
        .into_iter()
        .with_transformer::<ImageCaptionTransformer<_>>()
        .collect();
    assert_eq!(out.len(), 2);

    let second_html = match &out[1] {
        Event::Html(html) => html.to_string(),
        _ => panic!("expected Html"),
    };

    assert!(second_html.contains(r#"loading="lazy""#));
    assert!(!second_html.contains(r#"fetchpriority="high""#));
}
