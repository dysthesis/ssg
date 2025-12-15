//! Cold start benchmarks
//!
//! These benchmarks measure first-time initialization costs by spawning
//! fresh processes. They are report-only by default.

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main, measurement::WallTime,
};
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use util::load_snippet;

/// Configure a cold start benchmark group
fn configure_cold_start_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.02)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(10))
        .sample_size(50);
}

fn cold_start_syntect_first_highlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_start_syntect_first_highlight");
    configure_cold_start_group(&mut group);

    // Find the cold_start_helper binary
    let helper_path = find_helper_binary();

    // Create a temporary file with the code snippet
    let code_snippet = load_snippet("code_rust_small.txt");
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let snippet_path = temp_dir.path().join("snippet.txt");
    std::fs::write(&snippet_path, &code_snippet).expect("Failed to write snippet");

    group.bench_with_input(
        BenchmarkId::from_parameter("rust_small"),
        &snippet_path,
        |b, path| {
            b.iter(|| {
                let output = Command::new(&helper_path)
                    .arg("syntect")
                    .arg(black_box(path))
                    .arg(black_box("rust"))
                    .output()
                    .expect("Failed to execute cold_start_helper");

                black_box(output);
            });
        },
    );

    group.finish();
}

fn cold_start_katex_first_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_start_katex_first_render");
    configure_cold_start_group(&mut group);

    // Find the cold_start_helper binary
    let helper_path = find_helper_binary();

    // Create a temporary file with the math snippet
    let math_snippet = load_snippet("math_simple.tex");
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let snippet_path = temp_dir.path().join("math.tex");
    std::fs::write(&snippet_path, &math_snippet).expect("Failed to write snippet");

    group.bench_with_input(
        BenchmarkId::from_parameter("simple_inline"),
        &snippet_path,
        |b, path| {
            b.iter(|| {
                let output = Command::new(&helper_path)
                    .arg("katex")
                    .arg(black_box(path))
                    .arg(black_box("false"))
                    .output()
                    .expect("Failed to execute cold_start_helper");

                black_box(output);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::from_parameter("simple_display"),
        &snippet_path,
        |b, path| {
            b.iter(|| {
                let output = Command::new(&helper_path)
                    .arg("katex")
                    .arg(black_box(path))
                    .arg(black_box("true"))
                    .output()
                    .expect("Failed to execute cold_start_helper");

                black_box(output);
            });
        },
    );

    group.finish();
}

/// Find the cold_start_helper binary in the target directory
fn find_helper_binary() -> PathBuf {
    // Try release first, then debug
    let candidates = [
        "target/release/cold_start_helper",
        "target/debug/cold_start_helper",
    ];

    for candidate in &candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return path;
        }
    }

    panic!(
        "Could not find cold_start_helper binary. Please build it first with: \
         cargo build --bin cold_start_helper --release"
    );
}

criterion_group!(
    cold_start_benches,
    cold_start_syntect_first_highlight,
    cold_start_katex_first_render
);
criterion_main!(cold_start_benches);
