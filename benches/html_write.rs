//! HTML write benchmarks
//!
//! These benchmarks measure:
//! - HTML document serialization to memory
//! - Output path computation

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use libssg::document::{Buildable, Document, Parseable, compute_output_path};
use std::hint::black_box;
use std::path::PathBuf;
use util::{load_corpus, load_snippet};

/// Configure an HTML write benchmark group
fn configure_html_write_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.02)
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(8))
        .sample_size(100);
}

fn html_serialisation_to_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_serialisation_to_memory");
    configure_html_write_group(&mut group);

    // Load stylesheets
    let no_stylesheet: Option<String> = None;
    let small_stylesheet = Some(load_snippet("style_small.css"));
    let large_stylesheet = Some(load_snippet("style_large.css"));

    // Load and pre-render documents
    let plain_64k = load_corpus("plain/64k.md");
    let plain_1m = load_corpus("plain/1m.md");

    // Helper to render and get HTML doc
    let make_html_doc = |corpus: &util::CorpusFile, stylesheet: Option<String>| {
        let doc = Document::new(PathBuf::from("test.md"), corpus.as_str(), stylesheet);
        let parsed = doc.parse();
        parsed.build()
    };

    // Helper to measure output size
    let measure_output_size = |html_doc: &libssg::document::HtmlDocument| -> usize {
        let mut buffer = Vec::new();
        html_doc.write_to(&mut buffer).unwrap();
        buffer.len()
    };

    // Benchmark 64k with no stylesheet
    let html_64k_none = make_html_doc(&plain_64k, no_stylesheet.clone());
    let output_size_64k_none = measure_output_size(&html_64k_none);

    group.throughput(Throughput::Bytes(output_size_64k_none as u64));
    group.bench_with_input(
        BenchmarkId::new("64k", "no_stylesheet"),
        &html_64k_none,
        |b, html_doc| {
            b.iter(|| {
                let mut buffer = Vec::new();
                html_doc.write_to(black_box(&mut buffer)).unwrap();
                black_box(buffer);
            });
        },
    );

    // Benchmark 64k with small stylesheet
    let html_64k_small = make_html_doc(&plain_64k, small_stylesheet.clone());
    let output_size_64k_small = measure_output_size(&html_64k_small);

    group.throughput(Throughput::Bytes(output_size_64k_small as u64));
    group.bench_with_input(
        BenchmarkId::new("64k", "small_stylesheet"),
        &html_64k_small,
        |b, html_doc| {
            b.iter(|| {
                let mut buffer = Vec::new();
                html_doc.write_to(black_box(&mut buffer)).unwrap();
                black_box(buffer);
            });
        },
    );

    // Benchmark 64k with large stylesheet
    let html_64k_large = make_html_doc(&plain_64k, large_stylesheet.clone());
    let output_size_64k_large = measure_output_size(&html_64k_large);

    group.throughput(Throughput::Bytes(output_size_64k_large as u64));
    group.bench_with_input(
        BenchmarkId::new("64k", "large_stylesheet"),
        &html_64k_large,
        |b, html_doc| {
            b.iter(|| {
                let mut buffer = Vec::new();
                html_doc.write_to(black_box(&mut buffer)).unwrap();
                black_box(buffer);
            });
        },
    );

    // Benchmark 1m with no stylesheet
    let html_1m_none = make_html_doc(&plain_1m, no_stylesheet);
    let output_size_1m_none = measure_output_size(&html_1m_none);

    group.throughput(Throughput::Bytes(output_size_1m_none as u64));
    group.bench_with_input(
        BenchmarkId::new("1m", "no_stylesheet"),
        &html_1m_none,
        |b, html_doc| {
            b.iter(|| {
                let mut buffer = Vec::new();
                html_doc.write_to(black_box(&mut buffer)).unwrap();
                black_box(buffer);
            });
        },
    );

    // Benchmark 1m with large stylesheet
    let html_1m_large = make_html_doc(&plain_1m, large_stylesheet);
    let output_size_1m_large = measure_output_size(&html_1m_large);

    group.throughput(Throughput::Bytes(output_size_1m_large as u64));
    group.bench_with_input(
        BenchmarkId::new("1m", "large_stylesheet"),
        &html_1m_large,
        |b, html_doc| {
            b.iter(|| {
                let mut buffer = Vec::new();
                html_doc.write_to(black_box(&mut buffer)).unwrap();
                black_box(buffer);
            });
        },
    );

    group.finish();
}

fn output_path_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("output_path_computation");
    configure_html_write_group(&mut group);

    // Test cases
    let working_dir = PathBuf::from("/home/user/projects/mysite");

    let test_cases = [
        (
            "shallow_relative",
            PathBuf::from("file.md"),
            working_dir.clone(),
        ),
        (
            "deep_relative",
            PathBuf::from("a/b/c/d/file.md"),
            working_dir.clone(),
        ),
        (
            "absolute_under_wd",
            working_dir.join("docs/file.md"),
            working_dir.clone(),
        ),
        (
            "absolute_outside_wd",
            PathBuf::from("/tmp/file.md"),
            working_dir.clone(),
        ),
    ];

    for (name, input_path, wd) in test_cases.iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(input_path, wd),
            |b, (path, wd)| {
                b.iter(|| {
                    let result = compute_output_path(black_box(path), black_box(wd));
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    html_write_benches,
    html_serialisation_to_memory,
    output_path_computation
);
criterion_main!(html_write_benches);
