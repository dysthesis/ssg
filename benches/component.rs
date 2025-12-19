//! Component benchmarks for parsing and rendering
//!
//! These benchmarks measure the performance of major components:
//! - Parsing events from markdown
//! - Rendering parsed events to HTML (without special features)

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use libssg::document::Html;
use libssg::document::{Document, Parseable};
use libssg::highlighter::CodeblockHighlighter;
use libssg::math::MathRenderer;
use libssg::transformer::code_block::ToCodeBlockTransformer;
use libssg::transformer::math::ToMathTransformer;
use pulldown_cmark::{Event, html};
use std::hint::black_box;
use std::path::PathBuf;
use util::load_corpus;

/// Configure a component benchmark group
fn configure_component_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.02)
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(14))
        .sample_size(100);
}

fn parse_events_plain(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_events_plain");
    configure_component_group(&mut group);

    // Load inputs
    let inputs = [
        ("1k", load_corpus("plain/1k.md")),
        ("64k", load_corpus("plain/64k.md")),
        ("1m", load_corpus("plain/1m.md")),
    ];

    for (size, corpus) in inputs.iter() {
        group.throughput(Throughput::Bytes(corpus.size_bytes() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &corpus.as_str(),
            |b, input| {
                b.iter(|| {
                    let doc = Document::new(
                        black_box(PathBuf::from("test.md")),
                        black_box(input),
                        black_box(None),
                    );
                    let parsed = doc.parse();
                    let events: Vec<Event> = black_box(parsed.iterator.collect());
                    black_box(events);
                });
            },
        );
    }

    group.finish();
}

fn parse_events_mixed_features(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_events_mixed_features");
    configure_component_group(&mut group);

    // Load inputs
    let inputs = [
        ("64k", load_corpus("mixed_features/64k.md")),
        ("1m", load_corpus("mixed_features/1m.md")),
    ];

    for (size, corpus) in inputs.iter() {
        group.throughput(Throughput::Bytes(corpus.size_bytes() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &corpus.as_str(),
            |b, input| {
                b.iter(|| {
                    let doc = Document::new(
                        black_box(PathBuf::from("test.md")),
                        black_box(input),
                        black_box(None),
                    );
                    let parsed = doc.parse();
                    let events: Vec<Event> = black_box(parsed.iterator.collect());
                    black_box(events);
                });
            },
        );
    }

    group.finish();
}

// Dummy implementations for no-op rendering
#[derive(Clone)]
struct NoOpHighlighter;

impl CodeblockHighlighter for NoOpHighlighter {
    fn render_codeblock(&self, _source: &str, _language: Option<&str>) -> Html {
        Html::from("")
    }
}

#[derive(Clone)]
struct NoOpMathRenderer;

impl MathRenderer for NoOpMathRenderer {
    fn render_math(&self, _source: &str, _display_mode: bool) -> Html {
        Html::from("")
    }
}

fn render_push_html_no_specials(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_push_html_no_specials");
    configure_component_group(&mut group);

    // Pre-parse events for benchmarking
    let inputs = [
        ("64k", load_corpus("plain/64k.md")),
        ("1m", load_corpus("plain/1m.md")),
    ];

    // Create highlighter and math renderer once outside the timed loop
    let highlighter = NoOpHighlighter;
    let math_renderer = NoOpMathRenderer;

    for (size, corpus) in inputs.iter() {
        // Pre-parse the events
        let doc = Document::new(PathBuf::from("test.md"), corpus.as_str(), None);
        let parsed = doc.parse();
        let events: Vec<Event> = parsed.iterator.collect();

        group.throughput(Throughput::Bytes(corpus.size_bytes() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &events, |b, events| {
            b.iter_batched(
                || events.clone(),
                |events| {
                    let transformed = events
                        .into_iter()
                        .highlight_code(&highlighter)
                        .render_math(&math_renderer);

                    let mut output = String::new();
                    html::push_html(&mut output, black_box(transformed));
                    let html_result = Html::from(output);
                    black_box(html_result);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    component_benches,
    parse_events_plain,
    parse_events_mixed_features,
    render_push_html_no_specials
);
criterion_main!(component_benches);
