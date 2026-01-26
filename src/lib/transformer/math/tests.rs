use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
use pulldown_cmark::{CowStr, Event};

use crate::transformer::{WithTransformer, math::MathTransformer};

#[test]
fn math_transformer_converts_math() {
    let mut runner = TestRunner::new(Config {
        failure_persistence: None,
        ..Config::default()
    });

    runner
        .run(&"[A-Za-z0-9 +\\-*/^()]{1,12}", |math| {
            let events = vec![
                Event::InlineMath(CowStr::from(math.clone())),
                Event::DisplayMath(CowStr::from(math.clone())),
            ];
            let out: Vec<_> = events
                .into_iter()
                .with_transformer::<MathTransformer<_>>()
                .collect();
            prop_assert!(matches!(out[0], Event::InlineHtml(_)));
            prop_assert!(matches!(out[1], Event::Html(_)));
            Ok(())
        })
        .unwrap();
}
