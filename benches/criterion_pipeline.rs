use criterion::{
    BatchSize, BenchmarkId, Criterion, SamplingMode, Throughput, black_box, criterion_group,
    criterion_main,
};

use ssg::pipeline::build_at;

mod fixtures;
use fixtures::{SiteOptions, make_site, secs};

fn bench_builds(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_build");
    group.sampling_mode(SamplingMode::Flat);
    group.warm_up_time(secs(5));
    group.measurement_time(secs(10));

    let scenarios = [
        (
            "small-blog",
            SiteOptions {
                posts: 10,
                body_bytes: 1_000,
                with_code: false,
                with_math: false,
                with_footnotes: false,
                with_images: false,
            },
        ),
        (
            "realistic",
            SiteOptions {
                posts: 60,
                body_bytes: 5_000,
                with_code: true,
                with_math: true,
                with_footnotes: true,
                with_images: true,
            },
        ),
        (
            "stress",
            SiteOptions {
                posts: 400,
                body_bytes: 15_000,
                with_code: true,
                with_math: true,
                with_footnotes: true,
                with_images: true,
            },
        ),
    ];

    for (name, opts) in scenarios {
        let total_bytes = (opts.posts * opts.body_bytes) as u64;
        group.throughput(Throughput::Bytes(total_bytes));

        group.bench_function(BenchmarkId::new("build_at", name), |b| {
            b.iter_batched(
                || make_site(&opts),
                |tmp| {
                    build_at(tmp.path()).expect("build succeeds");
                    black_box(tmp);
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

criterion_group!(benches, bench_builds);
criterion_main!(benches);
