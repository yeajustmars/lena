use criterion::{Criterion, criterion_group, criterion_main};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("stub_benchmark", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
