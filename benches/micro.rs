//! Micro benchmarks for individual functions
//!
//! These benchmarks measure the performance of small, isolated functions:
//! - escape_html throughput
//! - KaTeX fallback rendering
//! - Syntect fallback rendering

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use libssg::highlighter::{escape_html, syntect::fallback_plain};
use libssg::math::katex::fallback_plain_math;
use std::hint::black_box;
use util::{load_corpus, load_snippet};

/// Configure a micro benchmark group
fn configure_micro_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.02)
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(5))
        .sample_size(200);
}

fn micro_escape_html_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_escape_html_throughput");
    configure_micro_group(&mut group);

    // Load inputs
    let plain_8k = load_corpus("plain/8k.md");
    let adversarial_64k = load_corpus("adversarial/64k_escape_dense.md");

    // Benchmark plain 8k
    group.throughput(Throughput::Bytes(plain_8k.size_bytes() as u64));
    group.bench_with_input(
        BenchmarkId::new("plain", "8k"),
        &plain_8k.as_str(),
        |b, input| {
            b.iter(|| {
                let result = escape_html(black_box(input));
                black_box(result);
            });
        },
    );

    // Benchmark adversarial 64k (escape-dense)
    group.throughput(Throughput::Bytes(adversarial_64k.size_bytes() as u64));
    group.bench_with_input(
        BenchmarkId::new("escape_dense", "64k"),
        &adversarial_64k.as_str(),
        |b, input| {
            b.iter(|| {
                let result = escape_html(black_box(input));
                black_box(result);
            });
        },
    );

    group.finish();
}

fn micro_katex_fallback_plain_math(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_katex_fallback_plain_math");
    configure_micro_group(&mut group);

    // Load inputs
    let math_simple = load_snippet("math_simple.tex");
    let math_complex = load_snippet("math_complex.tex");

    // Benchmark simple math
    group.throughput(Throughput::Bytes(math_simple.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("simple", "inline"),
        &math_simple,
        |b, input| {
            b.iter(|| {
                let result = fallback_plain_math(black_box(input), black_box(false));
                black_box(result);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("simple", "display"),
        &math_simple,
        |b, input| {
            b.iter(|| {
                let result = fallback_plain_math(black_box(input), black_box(true));
                black_box(result);
            });
        },
    );

    // Benchmark complex math
    group.throughput(Throughput::Bytes(math_complex.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("complex", "inline"),
        &math_complex,
        |b, input| {
            b.iter(|| {
                let result = fallback_plain_math(black_box(input), black_box(false));
                black_box(result);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("complex", "display"),
        &math_complex,
        |b, input| {
            b.iter(|| {
                let result = fallback_plain_math(black_box(input), black_box(true));
                black_box(result);
            });
        },
    );

    group.finish();
}

fn micro_syntect_fallback_plain_code(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_syntect_fallback_plain_code");
    configure_micro_group(&mut group);

    // Load inputs
    let code_plain_large = load_snippet("code_plain_large.txt");

    // Benchmark with safe language token
    group.throughput(Throughput::Bytes(code_plain_large.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("safe_language", "rust"),
        &code_plain_large,
        |b, input| {
            b.iter(|| {
                let result = fallback_plain(black_box(input), black_box(Some("rust")));
                black_box(result);
            });
        },
    );

    // Benchmark with adversarial language token
    group.bench_with_input(
        BenchmarkId::new("adversarial_language", "<script>"),
        &code_plain_large,
        |b, input| {
            b.iter(|| {
                let result = fallback_plain(
                    black_box(input),
                    black_box(Some("<script>alert('xss')</script>")),
                );
                black_box(result);
            });
        },
    );

    // Benchmark with None language
    group.bench_with_input(
        BenchmarkId::new("no_language", "none"),
        &code_plain_large,
        |b, input| {
            b.iter(|| {
                let result = fallback_plain(black_box(input), black_box(None));
                black_box(result);
            });
        },
    );

    group.finish();
}

criterion_group!(
    micro_benches,
    micro_escape_html_throughput,
    micro_katex_fallback_plain_math,
    micro_syntect_fallback_plain_code
);
criterion_main!(micro_benches);
