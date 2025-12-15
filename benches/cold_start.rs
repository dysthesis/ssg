//! Cold start benchmarks
//!
//! These benchmarks measure first-time initialization costs by spawning
//! fresh processes. They are report-only by default.
//!
//! NOTE: These measurements include:
//! - Process spawn and loader costs
//! - Command-line argument parsing
//! - The actual initialization (Syntect or KaTeX setup)
//! - Process exit
//!
//! The `noop` benchmark measures the baseline overhead (everything except
//! the actual initialization). Subtract `noop` from `syntect` or `katex`
//! to get the isolated initialization cost.

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main, measurement::WallTime,
};
use std::hint::black_box;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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

fn cold_start_noop_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_start_noop_baseline");
    configure_cold_start_group(&mut group);

    // Find the cold_start_helper binary
    let helper_path = find_helper_binary();

    group.bench_function("noop", |b| {
        b.iter(|| {
            let status = Command::new(&helper_path)
                .arg("noop")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("Failed to execute cold_start_helper");

            black_box(status);
        });
    });

    group.finish();
}

fn cold_start_syntect_first_highlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_start_syntect_first_highlight");
    configure_cold_start_group(&mut group);

    // Find the cold_start_helper binary
    let helper_path = find_helper_binary();

    // Load the code snippet (no filesystem I/O during measurement)
    let code_snippet = load_snippet("code_rust_small.txt");

    group.bench_with_input(
        BenchmarkId::from_parameter("rust_small"),
        &code_snippet,
        |b, snippet| {
            b.iter(|| {
                let status = Command::new(&helper_path)
                    .arg("syntect")
                    .arg(black_box(snippet))
                    .arg(black_box("rust"))
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .expect("Failed to execute cold_start_helper");

                black_box(status);
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

    // Load the math snippet (no filesystem I/O during measurement)
    let math_snippet = load_snippet("math_simple.tex");

    group.bench_with_input(
        BenchmarkId::from_parameter("simple_inline"),
        &math_snippet,
        |b, snippet| {
            b.iter(|| {
                let status = Command::new(&helper_path)
                    .arg("katex")
                    .arg(black_box(snippet))
                    .arg(black_box("false"))
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .expect("Failed to execute cold_start_helper");

                black_box(status);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::from_parameter("simple_display"),
        &math_snippet,
        |b, snippet| {
            b.iter(|| {
                let status = Command::new(&helper_path)
                    .arg("katex")
                    .arg(black_box(snippet))
                    .arg(black_box("true"))
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .expect("Failed to execute cold_start_helper");

                black_box(status);
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
    cold_start_noop_baseline,
    cold_start_syntect_first_highlight,
    cold_start_katex_first_render
);
criterion_main!(cold_start_benches);
