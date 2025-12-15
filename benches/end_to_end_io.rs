//! End-to-end I/O-included benchmarks
//!
//! These benchmarks measure the complete pipeline including filesystem operations.
//! They should only be used for gating on stable, dedicated runners.

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use libssg::document::{Buildable, Document, Parseable, Writeable};
use std::env::{current_dir, set_current_dir};
use std::hint::black_box;
use std::path::PathBuf;
use tempfile::TempDir;
use util::load_corpus;

/// Guard that restores the working directory on drop
struct CwdGuard {
    original: PathBuf,
}

impl CwdGuard {
    fn enter(temp_dir: &TempDir) -> std::io::Result<Self> {
        let original = current_dir()?;
        set_current_dir(temp_dir.path())?;
        Ok(Self { original })
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = set_current_dir(&self.original);
    }
}

/// Configure an end-to-end I/O benchmark group
fn configure_e2e_io_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.02)
        .warm_up_time(std::time::Duration::from_secs(3))
        .measurement_time(std::time::Duration::from_secs(20))
        .sample_size(30);
}

fn e2e_io_site_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_io_site_small");
    configure_e2e_io_group(&mut group);

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
            b.iter_batched(
                || {
                    // Setup: create a fresh temporary directory
                    TempDir::new().unwrap()
                },
                |temp_dir| {
                    // Measured operation: change to temp directory and write files
                    let _guard = CwdGuard::enter(&temp_dir).expect("failed to change directory");

                    for (path, content) in black_box(docs) {
                        let doc = Document::new(path.clone(), black_box(content), black_box(None));
                        let parsed = doc.parse();
                        let html = parsed.build().expect("build should succeed");
                        html.write().expect("write failed");
                    }

                    black_box(temp_dir);
                },
                criterion::BatchSize::PerIteration,
            );
        },
    );

    group.finish();
}

criterion_group!(e2e_io_benches, e2e_io_site_small);
criterion_main!(e2e_io_benches);
