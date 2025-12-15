//! Renderer isolation benchmarks
//!
//! These benchmarks measure renderer translation logic with stubbed
//! highlighters and math renderers to isolate the overhead of the
//! renderer itself from external library costs.

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use libssg::document::{Document, Html, Parseable};
use libssg::renderer::{CodeblockHighlighter, MathRenderer, Renderer, escape_html};
use pulldown_cmark::Event;
use std::hint::black_box;
use std::path::PathBuf;
use util::load_corpus;

/// Configure a renderer isolation benchmark group
fn configure_renderer_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.02)
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(8))
        .sample_size(100);
}

// Minimal stub implementations that return constant HTML to isolate pure translation overhead
#[derive(Clone)]
struct MinimalStubHighlighter;

impl CodeblockHighlighter for MinimalStubHighlighter {
    fn render_codeblock(&self, _source: &str, _language: Option<&str>) -> Html {
        // Return a fixed-size constant to isolate translation overhead
        Html::from("<pre><code>...</code></pre>\n")
    }
}

#[derive(Clone)]
struct MinimalStubMathRenderer;

impl MathRenderer for MinimalStubMathRenderer {
    fn render_math(&self, _source: &str, display_mode: bool) -> Html {
        // Return a fixed-size constant to isolate translation overhead
        if display_mode {
            Html::from("<div>...</div>")
        } else {
            Html::from("<span>...</span>")
        }
    }
}

// Stub implementations that mimic the fallback structure with escaping
#[derive(Clone)]
struct StubHighlighter;

impl CodeblockHighlighter for StubHighlighter {
    fn render_codeblock(&self, source: &str, language: Option<&str>) -> Html {
        // Mimic the fallback structure but with predictable cost
        let mut out = String::with_capacity(source.len() + 32);
        out.push_str("<pre><code");
        if let Some(lang) = language {
            out.push_str(" class=\"language-");
            out.push_str(lang);
            out.push('"');
        }
        out.push('>');
        out.push_str(&escape_html(source));
        out.push_str("</code></pre>\n");
        Html::from(out)
    }
}

#[derive(Clone)]
struct StubMathRenderer;

impl MathRenderer for StubMathRenderer {
    fn render_math(&self, source: &str, display_mode: bool) -> Html {
        // Mimic the fallback structure
        let mut out = String::with_capacity(source.len() + 32);
        if display_mode {
            out.push_str("<div class=\"math math-display\">");
        } else {
            out.push_str("<span class=\"math math-inline\">");
        }
        out.push_str(&escape_html(source));
        if display_mode {
            out.push_str("</div>");
        } else {
            out.push_str("</span>");
        }
        Html::from(out)
    }
}

fn render_translation_minimal_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_translation_minimal_overhead");
    configure_renderer_group(&mut group);

    // Load inputs
    let code_dense_64k = load_corpus("code_dense/64k_40blocks.md");
    let math_dense_64k = load_corpus("math_dense/64k_valid.md");

    // Create renderer with minimal stubs to isolate pure translation overhead
    let renderer = Renderer::new(MinimalStubHighlighter, MinimalStubMathRenderer);

    // Pre-parse events for code_dense
    let doc = Document::new(PathBuf::from("test.md"), code_dense_64k.as_str(), None);
    let parsed = doc.parse();
    let code_events: Vec<Event> = parsed.iterator.collect();

    // Benchmark code_dense
    group.throughput(Throughput::Bytes(code_dense_64k.size_bytes() as u64));
    group.bench_with_input(
        BenchmarkId::new("code_dense", "64k"),
        &code_events,
        |b, events| {
            b.iter_batched(
                || events.clone(),
                |events| {
                    let html = renderer
                        .render(black_box(events))
                        .expect("render should succeed");
                    black_box(html);
                },
                criterion::BatchSize::SmallInput,
            );
        },
    );

    // Pre-parse events for math_dense
    let doc = Document::new(PathBuf::from("test.md"), math_dense_64k.as_str(), None);
    let parsed = doc.parse();
    let math_events: Vec<Event> = parsed.iterator.collect();

    // Benchmark math_dense
    group.throughput(Throughput::Bytes(math_dense_64k.size_bytes() as u64));
    group.bench_with_input(
        BenchmarkId::new("math_dense", "64k"),
        &math_events,
        |b, events| {
            b.iter_batched(
                || events.clone(),
                |events| {
                    let html = renderer
                        .render(black_box(events))
                        .expect("render should succeed");
                    black_box(html);
                },
                criterion::BatchSize::SmallInput,
            );
        },
    );

    group.finish();
}

fn render_translation_with_stub_highlighter_and_stub_math(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_translation_with_stub_highlighter_and_stub_math");
    configure_renderer_group(&mut group);

    // Load inputs
    let code_dense_64k = load_corpus("code_dense/64k_40blocks.md");
    let math_dense_64k = load_corpus("math_dense/64k_valid.md");

    // Create renderer with stubs that include escaping (comparable to fallback)
    let renderer = Renderer::new(StubHighlighter, StubMathRenderer);

    // Pre-parse events for code_dense
    let doc = Document::new(PathBuf::from("test.md"), code_dense_64k.as_str(), None);
    let parsed = doc.parse();
    let code_events: Vec<Event> = parsed.iterator.collect();

    // Benchmark code_dense
    group.throughput(Throughput::Bytes(code_dense_64k.size_bytes() as u64));
    group.bench_with_input(
        BenchmarkId::new("code_dense", "64k"),
        &code_events,
        |b, events| {
            b.iter_batched(
                || events.clone(),
                |events| {
                    let html = renderer
                        .render(black_box(events))
                        .expect("render should succeed");
                    black_box(html);
                },
                criterion::BatchSize::SmallInput,
            );
        },
    );

    // Pre-parse events for math_dense
    let doc = Document::new(PathBuf::from("test.md"), math_dense_64k.as_str(), None);
    let parsed = doc.parse();
    let math_events: Vec<Event> = parsed.iterator.collect();

    // Benchmark math_dense
    group.throughput(Throughput::Bytes(math_dense_64k.size_bytes() as u64));
    group.bench_with_input(
        BenchmarkId::new("math_dense", "64k"),
        &math_events,
        |b, events| {
            b.iter_batched(
                || events.clone(),
                |events| {
                    let html = renderer
                        .render(black_box(events))
                        .expect("render should succeed");
                    black_box(html);
                },
                criterion::BatchSize::SmallInput,
            );
        },
    );

    group.finish();
}

fn render_with_syntect_warm(c: &mut Criterion) {
    use libssg::renderer::syntect::SyntectHighlighter;

    let mut group = c.benchmark_group("render_with_syntect_warm");
    configure_renderer_group(&mut group);

    // Load inputs
    let code_rust_small = util::load_snippet("code_rust_small.txt");
    let code_rust_large = util::load_snippet("code_rust_large.txt");

    let highlighter = SyntectHighlighter::default();

    // Force initialization (warm-up)
    let _ = highlighter.render_codeblock(&code_rust_small, Some("rust"));

    // Benchmark small rust code
    group.throughput(Throughput::Bytes(code_rust_small.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("rust", "small"),
        &code_rust_small,
        |b, input| {
            b.iter(|| {
                let result =
                    highlighter.render_codeblock(black_box(input), black_box(Some("rust")));
                black_box(result);
            });
        },
    );

    // Benchmark large rust code
    group.throughput(Throughput::Bytes(code_rust_large.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("rust", "large"),
        &code_rust_large,
        |b, input| {
            b.iter(|| {
                let result =
                    highlighter.render_codeblock(black_box(input), black_box(Some("rust")));
                black_box(result);
            });
        },
    );

    // Benchmark with unknown language
    group.bench_with_input(
        BenchmarkId::new("unknown", "large"),
        &code_rust_large,
        |b, input| {
            b.iter(|| {
                let result = highlighter
                    .render_codeblock(black_box(input), black_box(Some("unknown_language")));
                black_box(result);
            });
        },
    );

    // Benchmark with adversarial language token
    group.bench_with_input(
        BenchmarkId::new("adversarial", "large"),
        &code_rust_large,
        |b, input| {
            b.iter(|| {
                let result = highlighter.render_codeblock(
                    black_box(input),
                    black_box(Some("<script>alert('xss')</script>")),
                );
                black_box(result);
            });
        },
    );

    group.finish();
}

fn render_with_katex_warm_success_and_fallback(c: &mut Criterion) {
    use libssg::renderer::katex::KatexRenderer;

    // Success cases
    {
        let mut group = c.benchmark_group("render_with_katex_warm_success");
        configure_renderer_group(&mut group);

        let math_simple = util::load_snippet("math_simple.tex");
        let math_complex = util::load_snippet("math_complex.tex");

        let renderer = KatexRenderer::new();

        // Force initialization
        let _ = renderer.render_math(&math_simple, false);

        // Benchmark simple inline
        group.throughput(Throughput::Bytes(math_simple.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("simple", "inline"),
            &math_simple,
            |b, input| {
                b.iter(|| {
                    let result = renderer.render_math(black_box(input), black_box(false));
                    black_box(result);
                });
            },
        );

        // Benchmark simple display
        group.bench_with_input(
            BenchmarkId::new("simple", "display"),
            &math_simple,
            |b, input| {
                b.iter(|| {
                    let result = renderer.render_math(black_box(input), black_box(true));
                    black_box(result);
                });
            },
        );

        // Benchmark complex inline
        group.throughput(Throughput::Bytes(math_complex.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("complex", "inline"),
            &math_complex,
            |b, input| {
                b.iter(|| {
                    let result = renderer.render_math(black_box(input), black_box(false));
                    black_box(result);
                });
            },
        );

        // Benchmark complex display
        group.bench_with_input(
            BenchmarkId::new("complex", "display"),
            &math_complex,
            |b, input| {
                b.iter(|| {
                    let result = renderer.render_math(black_box(input), black_box(true));
                    black_box(result);
                });
            },
        );

        group.finish();
    }

    // Fallback cases
    {
        let mut group = c.benchmark_group("render_with_katex_warm_fallback");
        configure_renderer_group(&mut group);

        let math_invalid = util::load_snippet("math_invalid.tex");

        let renderer = KatexRenderer::new();

        // Benchmark invalid inline
        group.throughput(Throughput::Bytes(math_invalid.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("invalid", "inline"),
            &math_invalid,
            |b, input| {
                b.iter(|| {
                    let result = renderer.render_math(black_box(input), black_box(false));
                    black_box(result);
                });
            },
        );

        // Benchmark invalid display
        group.bench_with_input(
            BenchmarkId::new("invalid", "display"),
            &math_invalid,
            |b, input| {
                b.iter(|| {
                    let result = renderer.render_math(black_box(input), black_box(true));
                    black_box(result);
                });
            },
        );

        group.finish();
    }
}

criterion_group!(
    renderer_isolation_benches,
    render_translation_minimal_overhead,
    render_translation_with_stub_highlighter_and_stub_math,
    render_with_syntect_warm,
    render_with_katex_warm_success_and_fallback
);
criterion_main!(renderer_isolation_benches);
