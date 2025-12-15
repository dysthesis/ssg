use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
// use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("fibonacci_recursive");
    for n in [10u64, 15, 20] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            // TODO: add actual thing to benchmark
            b.iter(|| println!("Hello world!"))
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
