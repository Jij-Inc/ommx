// Create partial evaluation benchmarks for Linear
use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};

use ommx::{
    random::{arbitrary_state, random_deterministic, sample_deterministic, FunctionParameters},
    v1::Linear,
    Evaluate,
};

fn partial_evaluate_linear_all(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("partial-evaluate-linear-all");
    group.plot_config(plot_config.clone());
    for num_terms in [10usize, 100, 1_000, 10_000] {
        let lin: Linear = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 1,
            max_id: 10 * num_terms as u64,
        });
        let ids = lin.used_decision_variable_ids();
        let state = sample_deterministic(arbitrary_state(ids));
        group.bench_with_input(
            BenchmarkId::new("partial-evaluate-linear-all", num_terms.to_string()),
            &(lin, state),
            |b, (lin, state)| {
                b.iter(|| {
                    let mut f = lin.clone();
                    f.partial_evaluate(&state).unwrap();
                })
            },
        );
    }
    group.finish();
}

fn partial_evaluate_linear_half(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("partial-evaluate-linear-half");
    group.plot_config(plot_config.clone());
    for num_terms in [10usize, 100, 1_000, 10_000] {
        let lin: Linear = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 1,
            max_id: 10 * num_terms as u64,
        });
        let ids: Vec<_> = lin.used_decision_variable_ids().into_iter().collect();
        let half_count = ids.len() / 2;
        let half_ids: std::collections::BTreeSet<_> = ids.into_iter().take(half_count).collect();
        let state = sample_deterministic(arbitrary_state(half_ids));
        group.bench_with_input(
            BenchmarkId::new("partial-evaluate-linear-half", num_terms.to_string()),
            &(lin, state),
            |b, (lin, state)| {
                b.iter(|| {
                    let mut f = lin.clone();
                    f.partial_evaluate(&state).unwrap();
                })
            },
        );
    }
    group.finish();
}

fn partial_evaluate_linear_one(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("partial-evaluate-linear-one");
    group.plot_config(plot_config.clone());
    for num_terms in [10usize, 100, 1_000, 10_000] {
        let lin: Linear = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 1,
            max_id: 10 * num_terms as u64,
        });
        let one_ids: std::collections::BTreeSet<_> = lin
            .used_decision_variable_ids()
            .into_iter()
            .take(1)
            .collect();
        let state = sample_deterministic(arbitrary_state(one_ids));
        group.bench_with_input(
            BenchmarkId::new("partial-evaluate-linear-one", num_terms.to_string()),
            &(lin, state),
            |b, (lin, state)| {
                b.iter(|| {
                    let mut f = lin.clone();
                    f.partial_evaluate(&state).unwrap();
                })
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    partial_evaluate_linear_all,
    partial_evaluate_linear_half,
    partial_evaluate_linear_one,
);
criterion_main!(benches);
