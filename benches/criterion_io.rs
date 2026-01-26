use std::io::Write;

use brotli::CompressorWriter;
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use flate2::{Compression, write::GzEncoder};
use minify_html::{Cfg, minify};
use pulldown_cmark::{Options, Parser};

mod fixtures;
use fixtures::{rust_snippet, secs};

fn make_min_cfg() -> Cfg {
    let mut cfg = Cfg::new();
    cfg.minify_css = false;
    cfg.minify_js = true;
    cfg.allow_optimal_entities = true;
    cfg.allow_noncompliant_unquoted_attribute_values = true;
    cfg.allow_removing_spaces_between_attributes = true;
    cfg.minify_doctype = true;
    cfg.remove_bangs = true;
    cfg.remove_processing_instructions = true;
    cfg.keep_closing_tags = false;
    cfg.keep_comments = false;
    cfg.keep_html_and_head_opening_tags = false;
    cfg
}

fn bench_minify_html(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_minify_html");
    let html = format!(
        "<html><body>{}</body></html>",
        "<p>Lorem ipsum dolor sit amet.</p>".repeat(8_000)
    );
    let cfg = make_min_cfg();
    group.throughput(Throughput::Bytes(html.len() as u64));
    group.bench_function("minify", |b| {
        b.iter(|| {
            let out = minify(html.as_bytes(), &cfg);
            black_box(out);
        })
    });
    group.finish();
}

fn bench_markdown_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_markdown_parse");
    let md = format!(
        "# Heading\n\n{}\n{}",
        rust_snippet(400),
        "Paragraph text.\n\n".repeat(2_000)
    );
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_GFM);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    group.throughput(Throughput::Bytes(md.len() as u64));
    group.warm_up_time(secs(2));

    group.bench_function("pulldown_cmark", |b| {
        b.iter(|| {
            let parser = Parser::new_ext(&md, opts);
            let events: Vec<_> = parser.collect();
            black_box(events);
        })
    });

    group.finish();
}

fn bench_compress(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_compress");
    let data = rust_snippet(10_000); // ~ large text
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function(BenchmarkId::new("gzip_best", "text"), |b| {
        b.iter(|| {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
            encoder.write_all(data.as_bytes()).unwrap();
            let out = encoder.finish().unwrap();
            black_box(out);
        })
    });

    group.bench_function(BenchmarkId::new("brotli_q11", "text"), |b| {
        b.iter(|| {
            let mut writer = CompressorWriter::new(Vec::new(), 4096, 11, 22);
            writer.write_all(data.as_bytes()).unwrap();
            let out = writer.into_inner();
            black_box(out);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_minify_html,
    bench_markdown_parse,
    bench_compress
);
criterion_main!(benches);
