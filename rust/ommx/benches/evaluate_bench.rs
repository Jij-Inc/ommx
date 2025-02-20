use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use ommx::{
    v1::{Linear, State},
    Evaluate,
};

fn bench_linear(c: &mut Criterion) {
    let mut sum_partial = c.benchmark_group("sum-partial");
    for n in (1..=5).map(|n| n * 2000) {
        sum_partial.bench_with_input(
            BenchmarkId::new("sum-partial", format!("{n:05}")),
            &n,
            |b, n| {
                let state = State::default();
                let mut lin = Linear::new((0..*n).map(|i| (i, 1.0)), 0.0);
                b.iter(|| lin.partial_evaluate(&state))
            },
        );
    }
    sum_partial.finish();

    let mut sum_total = c.benchmark_group("sum-total");
    for n in (1..=5).map(|n| n * 2000) {
        sum_total.bench_with_input(
            BenchmarkId::new("sum-total", format!("{n:05}")),
            &n,
            |b, n| {
                let mut state = State::default();
                state.entries = (0..*n).map(|i| (i, i as f64)).collect();
                let mut lin = Linear::new((0..*n).map(|i| (i, 1.0)), 0.0);
                b.iter(|| lin.partial_evaluate(&state))
            },
        );
    }
    sum_total.finish();
}

criterion_group!(eval_benches, bench_linear);

criterion_main!(eval_benches);
