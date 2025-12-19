//! I/O-specific Criterion benchmarks for the site build pipeline
//!
//! This benchmark suite decomposes and measures, with minimal confounding,
//! the filesystem-relevant costs of the site build pipeline.

mod util;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use itertools::Either;
use libssg::document::{Buildable, Document, Html, HtmlDocument, Parseable, compute_output_path};
use libssg::highlighter::escape_html;
use rayon::ThreadPoolBuilder;
use std::fs::{File, create_dir_all, read_to_string};
use std::hint::black_box;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use util::load_corpus;
use walkdir::WalkDir;

/// Check if extended benchmarks are enabled
fn is_extended_enabled() -> bool {
    std::env::var("LIBSSG_BENCH_IO_EXTENDED").is_ok()
}

/// Check if fsync benchmarks are enabled
fn is_fsync_enabled() -> bool {
    std::env::var("LIBSSG_BENCH_IO_FSYNC").is_ok()
}

/// File counts for stable grid
const STABLE_FILE_COUNTS: &[usize] = &[10, 100, 1000];

/// Directory depths for stable grid
const STABLE_DEPTHS: &[usize] = &[1, 4, 8];

/// Thread counts for stable grid
const STABLE_THREAD_COUNTS: &[usize] = &[1, 8];

/// Extended file counts
const EXTENDED_FILE_COUNTS: &[usize] = &[10, 100, 1000];

/// Extended depths
const EXTENDED_DEPTHS: &[usize] = &[1, 4, 8];

/// Extended thread counts
const EXTENDED_THREAD_COUNTS: &[usize] = &[1, 2, 4, 8];

/// Get file counts based on environment
fn file_counts() -> &'static [usize] {
    if is_extended_enabled() {
        EXTENDED_FILE_COUNTS
    } else {
        STABLE_FILE_COUNTS
    }
}

/// Get depths based on environment
fn depths() -> &'static [usize] {
    if is_extended_enabled() {
        EXTENDED_DEPTHS
    } else {
        STABLE_DEPTHS
    }
}

/// Get thread counts based on environment
fn thread_counts() -> &'static [usize] {
    if is_extended_enabled() {
        EXTENDED_THREAD_COUNTS
    } else {
        STABLE_THREAD_COUNTS
    }
}

/// Durability modes for writes
#[derive(Debug, Clone, Copy)]
enum DurabilityMode {
    Flush,
    SyncAll,
}

impl DurabilityMode {
    fn as_str(&self) -> &'static str {
        match self {
            DurabilityMode::Flush => "flush",
            DurabilityMode::SyncAll => "sync_all",
        }
    }
}

/// Get available durability modes
fn durability_modes() -> Vec<DurabilityMode> {
    let mut modes = vec![DurabilityMode::Flush];
    if is_fsync_enabled() {
        modes.push(DurabilityMode::SyncAll);
    }
    modes
}

/// Generate a deterministic relative path for a document
///
/// Uses a fixed fan-out of 16 directories per level, derived from `i` in hexadecimal.
/// For level k, the directory name is the k-th nybble of `i` rendered as a
/// two-character lower-case hex string. The filename is `doc_{i:08x}.{ext}`.
///
/// # Example
///
/// For i = 0x1a2b3c4d, depth = 3, ext = "md":
/// Returns: `1a/2b/3c/doc_1a2b3c4d.md`
fn doc_rel_path(i: usize, depth: usize, ext: &str) -> PathBuf {
    let depth = if depth == 0 { 1 } else { depth };

    let mut path = PathBuf::new();

    // Extract nybbles and build directory structure
    for level in (0..depth).rev() {
        let shift = level * 4;
        let nybble = ((i >> shift) & 0xF) as u8;
        let dir_name = format!("{:02x}", nybble);
        path.push(dir_name);
    }

    // Add filename
    let filename = format!("doc_{:08x}.{}", i, ext);
    path.push(filename);

    path
}

/// A materialised site dataset
#[derive(Debug)]
#[allow(dead_code)]
struct MaterialisedSite {
    root: PathBuf,
    md_paths: Vec<PathBuf>,
    abs_md_paths: Vec<PathBuf>,
    expected_md_bytes_total: usize,
    expected_entries_total: usize,
}

/// Create a deterministic site dataset under a temp root
fn materialise_site(
    // The root directory to create the site in
    root: &Path,
    // Number of Markdown files to create
    n_md: usize,
    // Directory depth for the site structure
    depth: usize,
    // Content for each Markdown file
    md_content: &str,
    // Number of noise files per Markdown file
    noise_factor: usize,
) -> std::io::Result<MaterialisedSite> {
    let mut md_paths = Vec::with_capacity(n_md);
    let mut abs_md_paths = Vec::with_capacity(n_md);
    let mut dir_count = 0;
    let mut dirs_created = std::collections::HashSet::new();

    // Create Markdown files
    for i in 0..n_md {
        let rel_path = doc_rel_path(i, depth, "md");
        let abs_path = root.join(&rel_path);

        // Create parent directories
        if let Some(parent) = abs_path.parent() {
            if !dirs_created.contains(parent) {
                create_dir_all(parent)?;
                dirs_created.insert(parent.to_path_buf());
                dir_count += 1;
            }
        }

        // Write Markdown file
        std::fs::write(&abs_path, md_content)?;

        md_paths.push(rel_path);
        abs_md_paths.push(abs_path);
    }

    // Create noise files
    let noise_content = "x".repeat(128);
    let n_noise = n_md * noise_factor;
    for i in 0..n_noise {
        let rel_path = doc_rel_path(i, depth, "txt");
        let abs_path = root.join(&rel_path);

        // Create parent directories if needed
        if let Some(parent) = abs_path.parent() {
            if !dirs_created.contains(parent) {
                create_dir_all(parent)?;
                dirs_created.insert(parent.to_path_buf());
                dir_count += 1;
            }
        }

        std::fs::write(&abs_path, &noise_content)?;
    }

    let expected_md_bytes_total = n_md * md_content.len();
    let expected_entries_total = n_md + n_noise + dir_count;

    Ok(MaterialisedSite {
        root: root.to_path_buf(),
        md_paths,
        abs_md_paths,
        expected_md_bytes_total,
        expected_entries_total,
    })
}

/// Generate HTML bytes for write benchmarks without heavy rendering. Creates an
/// HtmlDocument directly without Syntect or KaTeX initialisation
fn generate_html_bytes(md_path: &Path, md_content: &str) -> Vec<u8> {
    let body = Html::from(format!("<pre>{}</pre>\n", escape_html(md_content)));

    let html_doc = HtmlDocument::new(md_path.to_path_buf(), body, None, false);

    let mut bytes = Vec::new();
    html_doc
        .write_to(&mut bytes)
        .expect("write to vec should not fail");
    bytes
}

/// A write job for benchmarking
#[derive(Debug, Clone)]
struct WriteJob {
    output_path: PathBuf,
    html_bytes: Vec<u8>,
}

/// Prepare write jobs for benchmarking
fn prepare_write_jobs(root: &Path, md_paths: &[PathBuf], md_content: &str) -> Vec<WriteJob> {
    let working_dir = Path::new("/work");

    md_paths
        .iter()
        .map(|rel_md_path| {
            let rel_out = compute_output_path(rel_md_path, working_dir).expect("output path");
            let output_path = root.join(&rel_out);
            let html_bytes = generate_html_bytes(rel_md_path, md_content);

            WriteJob {
                output_path,
                html_bytes,
            }
        })
        .collect()
}

fn configure_enumerate_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.03)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(20))
        .sample_size(30);
}

fn configure_read_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.03)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(20))
        .sample_size(30);
}

fn configure_write_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.03)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(30))
        .sample_size(20);
}

fn configure_pipeline_group(group: &mut BenchmarkGroup<WallTime>) {
    group
        .confidence_level(0.99)
        .significance_level(0.01)
        .noise_threshold(0.03)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(30))
        .sample_size(20);
}

fn io_enumerate_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_enumerate_tree");
    configure_enumerate_group(&mut group);

    let md_content = load_corpus("plain/8k.md").content;

    for &n in file_counts() {
        for &depth in depths() {
            let bench_id = format!("N={},depth={},noise=1x", n, depth);

            // Setup outside timed region
            let temp_dir = TempDir::new().expect("failed to create temp dir");
            let site = materialise_site(
                temp_dir.path(),
                n,
                depth,
                &md_content,
                1, // noise_factor
            )
            .expect("failed to materialise site");

            group.throughput(Throughput::Elements(n as u64));
            group.bench_with_input(BenchmarkId::from_parameter(&bench_id), &site, |b, site| {
                b.iter(|| {
                    use itertools::Itertools;

                    let (dir_entries, _errors): (Vec<_>, Vec<_>) = WalkDir::new(&site.root)
                        .into_iter()
                        .partition_map(|r| match r {
                            Ok(v) => Either::Left(v),
                            Err(e) => Either::Right(e),
                        });

                    let md_count = dir_entries
                        .into_iter()
                        .filter(|e| e.file_type().is_file())
                        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                        .count();

                    black_box(md_count)
                });
            });
        }
    }

    group.finish();
}

fn io_read_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_read_tree");
    configure_read_group(&mut group);

    // Test with both 8k and 64k files
    for corpus_path in &["plain/8k.md", "plain/64k.md"] {
        let _md_content_check = load_corpus(corpus_path).content;
        let size_label = if corpus_path.contains("8k") {
            "8k"
        } else {
            "64k"
        };

        // Add 1m for extended mode
        let test_with_1m = is_extended_enabled() && corpus_path.contains("64k");
        let corpus_paths = if test_with_1m {
            vec![*corpus_path, "plain/1m.md"]
        } else {
            vec![*corpus_path]
        };

        for actual_corpus_path in corpus_paths {
            let md_content = load_corpus(actual_corpus_path).content;
            let size_label = if actual_corpus_path.contains("1m") {
                "1m"
            } else {
                size_label
            };

            for &n in &[10, 100] {
                for &depth in &[1, 4] {
                    // Variant 1: read_to_string
                    {
                        let bench_id = format!(
                            "N={},depth={},size={},mode=read_to_string",
                            n, depth, size_label
                        );

                        let temp_dir = TempDir::new().expect("failed to create temp dir");
                        let site = materialise_site(
                            temp_dir.path(),
                            n,
                            depth,
                            &md_content,
                            0, // no noise for read benchmarks
                        )
                        .expect("failed to materialise site");

                        let total_bytes = site.expected_md_bytes_total;
                        group.throughput(Throughput::Bytes(total_bytes as u64));

                        group.bench_with_input(
                            BenchmarkId::from_parameter(&bench_id),
                            &site,
                            |b, site| {
                                b.iter(|| {
                                    let mut sum_len = 0;
                                    for path in &site.abs_md_paths {
                                        let s = read_to_string(path).expect("read failed");
                                        sum_len += s.len();
                                    }
                                    black_box(sum_len)
                                });
                            },
                        );
                    }

                    {
                        let bench_id = format!(
                            "N={},depth={},size={},mode=read_to_end_reuse_buffer",
                            n, depth, size_label
                        );

                        let temp_dir = TempDir::new().expect("failed to create temp dir");
                        let site = materialise_site(temp_dir.path(), n, depth, &md_content, 0)
                            .expect("failed to materialise site");

                        let total_bytes = site.expected_md_bytes_total;
                        group.throughput(Throughput::Bytes(total_bytes as u64));

                        group.bench_with_input(
                            BenchmarkId::from_parameter(&bench_id),
                            &site,
                            |b, site| {
                                b.iter(|| {
                                    let mut buffer = Vec::with_capacity(md_content.len());
                                    let mut sum_len = 0;

                                    for path in &site.abs_md_paths {
                                        buffer.clear();
                                        let mut file = File::open(path).expect("open failed");
                                        file.read_to_end(&mut buffer).expect("read failed");
                                        sum_len += buffer.len();
                                    }

                                    black_box(sum_len)
                                });
                            },
                        );
                    }
                }
            }
        }
    }

    group.finish();
}

fn io_write_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_write_tree");
    configure_write_group(&mut group);

    for corpus_path in &["plain/8k.md", "plain/64k.md"] {
        let md_content = load_corpus(corpus_path).content;
        let size_label = if corpus_path.contains("8k") {
            "8k"
        } else {
            "64k"
        };

        for &n in &[100] {
            for &depth in &[4] {
                for durability in durability_modes() {
                    // Variant 1: precreated directories
                    {
                        let bench_id = format!(
                            "N={},depth={},size={},write_dirs=precreated,durability={}",
                            n,
                            depth,
                            size_label,
                            durability.as_str()
                        );

                        let temp_dir = TempDir::new().expect("failed to create temp dir");
                        let site = materialise_site(temp_dir.path(), n, depth, &md_content, 0)
                            .expect("failed to materialise site");

                        let jobs = prepare_write_jobs(temp_dir.path(), &site.md_paths, &md_content);

                        // Collect unique parent directories
                        let mut dirs_to_create = std::collections::HashSet::new();
                        for job in &jobs {
                            if let Some(parent) = job
                                .output_path
                                .strip_prefix(temp_dir.path())
                                .ok()
                                .and_then(|p| p.parent())
                            {
                                dirs_to_create.insert(parent.to_path_buf());
                            }
                        }

                        let total_bytes: usize = jobs.iter().map(|j| j.html_bytes.len()).sum();
                        group.throughput(Throughput::Bytes(total_bytes as u64));

                        let iteration_counter = std::sync::atomic::AtomicUsize::new(0);

                        group.bench_with_input(
                            BenchmarkId::from_parameter(&bench_id),
                            &(&jobs, &iteration_counter, &dirs_to_create),
                            |b, (jobs, counter, dirs)| {
                                b.iter_custom(|iters| {
                                    let mut total_duration = Duration::default();

                                    for _ in 0..iters {
                                        let iter_num = counter
                                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                        let out_root =
                                            temp_dir.path().join(format!("out_run_{}", iter_num));
                                        create_dir_all(&out_root)
                                            .expect("failed to create out root");

                                        // Precreate all output directories (outside timed region)
                                        for dir in *dirs {
                                            create_dir_all(out_root.join(dir))
                                                .expect("failed to create dir");
                                        }

                                        let start = Instant::now();

                                        let mut sum_written = 0;
                                        for job in *jobs {
                                            let output_path = out_root.join(
                                                job.output_path
                                                    .strip_prefix(temp_dir.path())
                                                    .unwrap(),
                                            );

                                            // Directories already created
                                            let file =
                                                File::create(&output_path).expect("create failed");
                                            let mut writer =
                                                BufWriter::with_capacity(64 * 1024, file);
                                            writer
                                                .write_all(&job.html_bytes)
                                                .expect("write failed");
                                            writer.flush().expect("flush failed");

                                            if matches!(durability, DurabilityMode::SyncAll) {
                                                writer.get_ref().sync_all().expect("sync failed");
                                            }

                                            sum_written += job.html_bytes.len();
                                        }

                                        let elapsed = start.elapsed();
                                        total_duration += elapsed;

                                        black_box(sum_written);
                                    }

                                    total_duration
                                });
                            },
                        );
                    }

                    {
                        let bench_id = format!(
                            "N={},depth={},size={},write_dirs=on_demand,durability={}",
                            n,
                            depth,
                            size_label,
                            durability.as_str()
                        );

                        let temp_dir = TempDir::new().expect("failed to create temp dir");
                        let site = materialise_site(temp_dir.path(), n, depth, &md_content, 0)
                            .expect("failed to materialise site");

                        let jobs = prepare_write_jobs(temp_dir.path(), &site.md_paths, &md_content);

                        let total_bytes: usize = jobs.iter().map(|j| j.html_bytes.len()).sum();
                        group.throughput(Throughput::Bytes(total_bytes as u64));

                        let iteration_counter = std::sync::atomic::AtomicUsize::new(0);

                        group.bench_with_input(
                            BenchmarkId::from_parameter(&bench_id),
                            &(&jobs, &iteration_counter),
                            |b, (jobs, counter)| {
                                b.iter_custom(|iters| {
                                    let mut total_duration = Duration::default();

                                    for _ in 0..iters {
                                        let iter_num = counter
                                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                        let out_root =
                                            temp_dir.path().join(format!("out_run_{}", iter_num));
                                        create_dir_all(&out_root)
                                            .expect("failed to create out root");

                                        let start = Instant::now();

                                        let mut sum_written = 0;
                                        for job in *jobs {
                                            let output_path = out_root.join(
                                                job.output_path
                                                    .strip_prefix(temp_dir.path())
                                                    .unwrap(),
                                            );

                                            // Create parent directories on demand
                                            if let Some(parent) = output_path.parent() {
                                                create_dir_all(parent)
                                                    .expect("create_dir_all failed");
                                            }

                                            let file =
                                                File::create(&output_path).expect("create failed");
                                            let mut writer =
                                                BufWriter::with_capacity(64 * 1024, file);
                                            writer
                                                .write_all(&job.html_bytes)
                                                .expect("write failed");
                                            writer.flush().expect("flush failed");

                                            if matches!(durability, DurabilityMode::SyncAll) {
                                                writer.get_ref().sync_all().expect("sync failed");
                                            }

                                            sum_written += job.html_bytes.len();
                                        }

                                        let elapsed = start.elapsed();
                                        total_duration += elapsed;

                                        black_box(sum_written);
                                    }

                                    total_duration
                                });
                            },
                        );
                    }
                }
            }
        }
    }

    group.finish();
}

fn io_pipeline_plain_read_build_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_pipeline_plain_read_build_write");
    configure_pipeline_group(&mut group);

    let md_content = load_corpus("plain/8k.md").content;

    for &n in &[100, 1000] {
        for &depth in &[4] {
            for &threads in thread_counts() {
                let pipeline_mode = if threads == 1 { "seq" } else { "par" };
                let bench_id = format!(
                    "N={},depth={},size=8k,threads={},pipeline={}",
                    n, depth, threads, pipeline_mode
                );

                let temp_dir = TempDir::new().expect("failed to create temp dir");
                let site = materialise_site(temp_dir.path(), n, depth, &md_content, 0)
                    .expect("failed to materialise site");

                let total_bytes = site.expected_md_bytes_total;
                group.throughput(Throughput::Bytes(total_bytes as u64));

                // Create benchmark-local thread pool
                let pool = if threads > 1 {
                    Some(
                        ThreadPoolBuilder::new()
                            .num_threads(threads)
                            .build()
                            .expect("failed to create thread pool"),
                    )
                } else {
                    None
                };

                let working_dir = Path::new("/work");
                let iteration_counter = std::sync::atomic::AtomicUsize::new(0);

                group.bench_with_input(
                    BenchmarkId::from_parameter(&bench_id),
                    &(&site, &pool, &iteration_counter),
                    |b, (site, pool, counter)| {
                        b.iter_custom(|iters| {
                            let mut total_duration = Duration::default();

                            for _ in 0..iters {
                                let iter_num =
                                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                let out_root =
                                    temp_dir.path().join(format!("pipeline_run_{}", iter_num));
                                create_dir_all(&out_root).expect("failed to create out root");

                                let start = Instant::now();

                                if let Some(pool) = pool {
                                    // Parallel variant
                                    use rayon::prelude::*;

                                    pool.install(|| {
                                        site.abs_md_paths
                                            .par_iter()
                                            .zip(site.md_paths.par_iter())
                                            .for_each(|(abs_path, rel_path)| {
                                                let content =
                                                    read_to_string(abs_path).expect("read failed");

                                                let doc =
                                                    Document::new(rel_path.clone(), &content, None);
                                                let parsed = doc.parse();
                                                let html_doc = parsed.build();

                                                // Compute output path
                                                let rel_out = compute_output_path(
                                                    rel_path,
                                                    working_dir,
                                                )
                                                .expect("output path");
                                                let output_path = out_root.join(&rel_out);

                                                // Create parent directories
                                                if let Some(parent) = output_path.parent() {
                                                    create_dir_all(parent)
                                                        .expect("create_dir_all failed");
                                                }

                                                // Write
                                                let file = File::create(&output_path)
                                                    .expect("create failed");
                                                let mut writer =
                                                    BufWriter::with_capacity(64 * 1024, file);
                                                html_doc
                                                    .write_to(&mut writer)
                                                    .expect("write failed");
                                                writer.flush().expect("flush failed");
                                            });
                                    });
                                } else {
                                    // Sequential variant
                                    for (abs_path, rel_path) in
                                        site.abs_md_paths.iter().zip(site.md_paths.iter())
                                    {
                                        let content =
                                            read_to_string(abs_path).expect("read failed");

                                        let doc = Document::new(rel_path.clone(), &content, None);
                                        let parsed = doc.parse();
                                        let html_doc = parsed.build();

                                        // Compute output path
                                        let rel_out = compute_output_path(rel_path, working_dir)
                                            .expect("output path");
                                        let output_path = out_root.join(&rel_out);

                                        // Create parent directories
                                        if let Some(parent) = output_path.parent() {
                                            create_dir_all(parent).expect("create_dir_all failed");
                                        }

                                        // Write
                                        let file =
                                            File::create(&output_path).expect("create failed");
                                        let mut writer = BufWriter::with_capacity(64 * 1024, file);
                                        html_doc.write_to(&mut writer).expect("write failed");
                                        writer.flush().expect("flush failed");
                                    }
                                }

                                let elapsed = start.elapsed();
                                total_duration += elapsed;

                                black_box(&out_root);
                            }

                            total_duration
                        });
                    },
                );
            }
        }
    }

    group.finish();
}

criterion_group!(
    io_benches,
    io_enumerate_tree,
    io_read_tree,
    io_write_tree,
    io_pipeline_plain_read_build_write
);
criterion_main!(io_benches);
