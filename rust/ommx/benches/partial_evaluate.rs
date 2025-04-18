// Create partial evaluation benchmarks for Linear
use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};

use ommx::{
    random::{arbitrary_state, random_deterministic, sample_deterministic, FunctionParameters},
    v1::Linear,
    Evaluate,
};

fn bench_partial_linear<F>(c: &mut Criterion, group_name: &str, select_ids: F)
where
    F: Fn(&Linear) -> std::collections::BTreeSet<u64> + Copy,
{
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group(group_name);
    group.plot_config(plot_config.clone());
    for num_terms in [10usize, 100, 1_000, 10_000] {
        let lin: Linear = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 1,
            max_id: 10 * num_terms as u64,
        });
        let ids = select_ids(&lin);
        let state = sample_deterministic(arbitrary_state(ids));
        group.bench_with_input(
            BenchmarkId::new(group_name, num_terms.to_string()),
            &(lin.clone(), state),
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

/// Substitute all decision variables in a linear function
fn partial_evaluate_linear_all(c: &mut Criterion) {
    bench_partial_linear(c, "partial-evaluate-linear-all", |lin| {
        lin.used_decision_variable_ids()
    });
}

/// Substitute half of the decision variables in a linear function
fn partial_evaluate_linear_half(c: &mut Criterion) {
    bench_partial_linear(c, "partial-evaluate-linear-half", |lin| {
        let ids: Vec<_> = lin.used_decision_variable_ids().into_iter().collect();
        let n = ids.len() / 2;
        ids.into_iter().take(n).collect()
    });
}

/// Substitute one decision variable in a linear function
fn partial_evaluate_linear_one(c: &mut Criterion) {
    bench_partial_linear(c, "partial-evaluate-linear-one", |lin| {
        lin.used_decision_variable_ids()
            .into_iter()
            .take(1)
            .collect()
    });
}

criterion_group!(
    benches,
    partial_evaluate_linear_all,
    partial_evaluate_linear_half,
    partial_evaluate_linear_one,
);
criterion_main!(benches);
