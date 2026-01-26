use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};

use ssg::transformer::{
    WithTransformer, code_block::CodeHighlightTransformer,
    footnote::convert_footnotes_to_sidenotes, image::ImageCaptionTransformer,
    math::MathTransformer, toc::insert_toc_and_heading_ids,
};

mod fixtures;
use fixtures::{
    code_block_events, footnote_events, heading_events, math_events, rust_snippet, secs,
};

fn bench_code_highlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("transformer_code_highlight");
    let code = rust_snippet(200);
    let events = code_block_events(&code);
    group.throughput(Throughput::Bytes(code.len() as u64));
    group.warm_up_time(secs(2));

    group.bench_function("fenced_rs", |b| {
        b.iter(|| {
            let out: Vec<_> = events
                .clone()
                .into_iter()
                .with_transformer::<CodeHighlightTransformer<_>>()
                .collect();
            black_box(out);
        })
    });

    group.finish();
}

fn bench_math(c: &mut Criterion) {
    let mut group = c.benchmark_group("transformer_math");
    let expr = "x^2 + y^2 = z^2";
    let events = math_events(expr);
    group.throughput(Throughput::Elements(2));

    group.bench_function("inline_and_display", |b| {
        b.iter(|| {
            let out: Vec<_> = events
                .clone()
                .into_iter()
                .with_transformer::<MathTransformer<_>>()
                .collect();
            black_box(out);
        })
    });

    group.finish();
}

fn bench_footnotes(c: &mut Criterion) {
    let mut group = c.benchmark_group("transformer_footnotes");
    let events = footnote_events(50);
    group.throughput(Throughput::Elements(events.len() as u64));

    group.bench_function("inline_sidenotes", |b| {
        b.iter(|| {
            let out = convert_footnotes_to_sidenotes(events.clone());
            black_box(out);
        })
    });

    group.finish();
}

fn bench_toc(c: &mut Criterion) {
    let mut group = c.benchmark_group("transformer_toc");
    let events = heading_events(40, 3);
    group.throughput(Throughput::Elements(events.len() as u64));

    group.bench_function("insert_toc_and_ids", |b| {
        b.iter(|| {
            let out = insert_toc_and_heading_ids(events.clone());
            black_box(out);
        })
    });

    group.finish();
}

fn bench_images(c: &mut Criterion) {
    let mut group = c.benchmark_group("transformer_image");
    let events = vec![
        Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url: CowStr::from("https://example.com/pic.png"),
            title: CowStr::from("pic"),
            id: CowStr::from(""),
        }),
        Event::Text(CowStr::from("Alt text caption")),
        Event::End(TagEnd::Image),
    ];
    group.throughput(Throughput::Elements(1));

    group.bench_function("remote_image_caption", |b| {
        b.iter(|| {
            let out: Vec<_> = events
                .clone()
                .into_iter()
                .with_transformer::<ImageCaptionTransformer<_>>()
                .collect();
            black_box(out);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_code_highlight,
    bench_math,
    bench_footnotes,
    bench_toc,
    bench_images
);
criterion_main!(benches);
