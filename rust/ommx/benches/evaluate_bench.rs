use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ommx::{
    v1::{Linear, State},
    Evaluate,
};

fn bench_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum");
    for n in 1..=100 {
        group.bench_with_input(BenchmarkId::new("sum", n), &n, |b, n| {
            b.iter(|| {
                evaluate_with(
                    black_box(&State::default()),
                    black_box(&Linear::new((0..*n).map(|i| (i, 1.0)), 0.0)),
                );
            })
        });
    }
    group.finish();
}

fn evaluate_with<E: Evaluate>(state: &State, item: &E) {
    let _ = item.evaluate(state);
}

criterion_group!(eval_benches, bench_linear);

criterion_main!(eval_benches);
