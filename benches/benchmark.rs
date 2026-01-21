use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ssg::fibonacci;
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("fibonacci_recursive");
    for n in [10u64, 15, 20] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| fibonacci(black_box(n)))
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
