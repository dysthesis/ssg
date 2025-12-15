//! End-to-end CPU-only benchmarks
//!
//! These benchmarks measure the complete pipeline from markdown to HTML
//! without touching the filesystem in the timed region.

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use libssg::document::process_documents_in_memory;
use std::hint::black_box;
use std::path::PathBuf;
use util::load_corpus;

/// Configure an end-to-end CPU benchmark group
fn configure_e2e_cpu_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.02)
        .warm_up_time(std::time::Duration::from_secs(3))
        .measurement_time(std::time::Duration::from_secs(82))
        .sample_size(50);
}

fn e2e_cpu_plain_site_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_cpu_plain_site_small");
    configure_e2e_cpu_group(&mut group);

    // Create 10 documents: mix of plain and mixed_features
    let documents: Vec<(PathBuf, String)> = vec![
        (PathBuf::from("doc1.md"), load_corpus("plain/8k.md").content),
        (PathBuf::from("doc2.md"), load_corpus("plain/8k.md").content),
        (
            PathBuf::from("doc3.md"),
            load_corpus("mixed_features/8k.md").content,
        ),
        (PathBuf::from("doc4.md"), load_corpus("plain/8k.md").content),
        (
            PathBuf::from("doc5.md"),
            load_corpus("mixed_features/8k.md").content,
        ),
        (PathBuf::from("doc6.md"), load_corpus("plain/8k.md").content),
        (
            PathBuf::from("doc7.md"),
            load_corpus("code_dense/8k_5blocks.md").content,
        ),
        (PathBuf::from("doc8.md"), load_corpus("plain/8k.md").content),
        (
            PathBuf::from("doc9.md"),
            load_corpus("math_dense/8k_valid.md").content,
        ),
        (
            PathBuf::from("doc10.md"),
            load_corpus("plain/8k.md").content,
        ),
    ];

    let total_bytes: usize = documents.iter().map(|(_, content)| content.len()).sum();

    group.throughput(Throughput::Bytes(total_bytes as u64));
    group.bench_with_input(
        BenchmarkId::from_parameter("10_documents"),
        &documents,
        |b, docs| {
            b.iter(|| {
                let results = process_documents_in_memory(black_box(docs), black_box(None));
                black_box(results);
            });
        },
    );

    group.finish();
}

fn e2e_cpu_site_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_cpu_site_medium");
    configure_e2e_cpu_group(&mut group);

    // Create 100 documents by repeating the pattern
    let base_docs = vec![
        ("plain/8k.md", load_corpus("plain/8k.md").content),
        ("plain/8k.md", load_corpus("plain/8k.md").content),
        (
            "mixed_features/8k.md",
            load_corpus("mixed_features/8k.md").content,
        ),
        ("plain/8k.md", load_corpus("plain/8k.md").content),
        (
            "mixed_features/8k.md",
            load_corpus("mixed_features/8k.md").content,
        ),
        ("plain/8k.md", load_corpus("plain/8k.md").content),
        (
            "code_dense/8k_5blocks.md",
            load_corpus("code_dense/8k_5blocks.md").content,
        ),
        ("plain/8k.md", load_corpus("plain/8k.md").content),
        (
            "math_dense/8k_valid.md",
            load_corpus("math_dense/8k_valid.md").content,
        ),
        ("plain/8k.md", load_corpus("plain/8k.md").content),
    ];

    let mut documents: Vec<(PathBuf, String)> = Vec::new();
    for i in 0..100 {
        let (_, content) = &base_docs[i % base_docs.len()];
        documents.push((PathBuf::from(format!("doc{}.md", i)), content.clone()));
    }

    let total_bytes: usize = documents.iter().map(|(_, content)| content.len()).sum();

    group.throughput(Throughput::Bytes(total_bytes as u64));
    group.bench_with_input(
        BenchmarkId::from_parameter("100_documents"),
        &documents,
        |b, docs| {
            b.iter(|| {
                let results = process_documents_in_memory(black_box(docs), black_box(None));
                black_box(results);
            });
        },
    );

    group.finish();
}

fn e2e_cpu_large_documents(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_cpu_large_documents");
    configure_e2e_cpu_group(&mut group);

    // Test 64k and 1m plain documents to catch scaling behavior
    let documents_64k: Vec<(PathBuf, String)> = vec![
        (
            PathBuf::from("plain_64k.md"),
            load_corpus("plain/64k.md").content,
        ),
        (
            PathBuf::from("mixed_64k.md"),
            load_corpus("mixed_features/64k.md").content,
        ),
    ];

    let total_bytes_64k: usize = documents_64k.iter().map(|(_, c)| c.len()).sum();

    group.throughput(Throughput::Bytes(total_bytes_64k as u64));
    group.bench_with_input(
        BenchmarkId::from_parameter("64k_documents"),
        &documents_64k,
        |b, docs| {
            b.iter(|| {
                let results = process_documents_in_memory(black_box(docs), black_box(None));
                black_box(results);
            });
        },
    );

    // Test 1m documents
    let documents_1m: Vec<(PathBuf, String)> = vec![
        (
            PathBuf::from("plain_1m.md"),
            load_corpus("plain/1m.md").content,
        ),
        (
            PathBuf::from("mixed_1m.md"),
            load_corpus("mixed_features/1m.md").content,
        ),
    ];

    let total_bytes_1m: usize = documents_1m.iter().map(|(_, c)| c.len()).sum();

    group.throughput(Throughput::Bytes(total_bytes_1m as u64));
    group.bench_with_input(
        BenchmarkId::from_parameter("1m_documents"),
        &documents_1m,
        |b, docs| {
            b.iter(|| {
                let results = process_documents_in_memory(black_box(docs), black_box(None));
                black_box(results);
            });
        },
    );

    group.finish();
}

fn e2e_cpu_stress_dense_features(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_cpu_stress_dense_features");
    configure_e2e_cpu_group(&mut group);

    // Test the largest code-dense and math-dense documents to catch scaling regressions
    // These stress the special feature rendering paths
    let code_dense_1m = load_corpus("code_dense/1m_200blocks.md");
    let math_dense_1m = load_corpus("math_dense/1m_mixed_valid_invalid.md");

    let documents: Vec<(PathBuf, String)> = vec![
        (
            PathBuf::from("code_dense_1m.md"),
            code_dense_1m.content.clone(),
        ),
        (
            PathBuf::from("math_dense_1m.md"),
            math_dense_1m.content.clone(),
        ),
    ];

    let total_bytes: usize = documents.iter().map(|(_, c)| c.len()).sum();

    group.throughput(Throughput::Bytes(total_bytes as u64));
    group.bench_with_input(
        BenchmarkId::from_parameter("1m_dense_features"),
        &documents,
        |b, docs| {
            b.iter(|| {
                let results = process_documents_in_memory(black_box(docs), black_box(None));
                black_box(results);
            });
        },
    );

    group.finish();
}

criterion_group!(
    e2e_cpu_benches,
    e2e_cpu_plain_site_small,
    e2e_cpu_site_medium,
    e2e_cpu_large_documents,
    e2e_cpu_stress_dense_features
);
criterion_main!(e2e_cpu_benches);
