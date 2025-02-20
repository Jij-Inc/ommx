use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use ommx::{
    v1::{Linear, State},
    Evaluate,
};

fn bench_linear(c: &mut Criterion) {
    let mut sum_partial = c.benchmark_group("sum-partial");
    for n in (1..=5).map(|n| n * 2000) {
        sum_partial.bench_with_input(
            BenchmarkId::new("sum-partial", format!("{n:03}")),
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
    sum_partial.finish();

    let mut sum_total = c.benchmark_group("sum-total");
    for n in (1..=5).map(|n| n * 2000) {
        sum_total.bench_with_input(
            BenchmarkId::new("sum-total", format!("{n:03}")),
            &n,
            |b, n| {
                b.iter(|| {
                    partial_eval_with(
                        black_box(&{
                            let mut state = State::default();
                            state.entries = (0..*n).map(|i| (i, i as f64)).collect();
                            state
                        }),
                        black_box(&mut Linear::new((0..*n).map(|i| (i, 1.0)), 0.0)),
                    );
                })
            },
        );
    }
    sum_total.finish();
}

fn partial_eval_with<E: Evaluate>(state: &State, item: &mut E) {
    let _ = item.partial_evaluate(state);
}

criterion_group!(eval_benches, bench_linear);

criterion_main!(eval_benches);
