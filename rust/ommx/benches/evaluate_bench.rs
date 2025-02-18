use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ommx::{
    v1::{Linear, State},
    Evaluate,
};

fn bench_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum-opaque");
    for n in 1..=100 {
        group.bench_with_input(
            BenchmarkId::new("sum-opaque", format!("{n:03}")),
            &n,
            |b, n| {
                b.iter(|| {
                    partial_eval_with(
                        black_box(&State::default()),
                        black_box(&mut Linear::new((0..*n).map(|i| (i, 1.0)), 0.0)),
                    );
                })
            },
        );
    }
    group.finish();
}

fn partial_eval_with<E: Evaluate>(state: &State, item: &mut E) {
    let _ = item.partial_evaluate(state);
}

criterion_group!(eval_benches, bench_linear);

criterion_main!(eval_benches);
