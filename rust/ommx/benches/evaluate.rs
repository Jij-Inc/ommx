use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};

use ommx::{
    random::{arbitrary_state, random_deterministic, sample_deterministic, FunctionParameters},
    v1::{Linear, Quadratic},
    Evaluate,
};

fn evaluate(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);

    let mut eval_linear = c.benchmark_group("eval-linear");
    eval_linear.plot_config(plot_config.clone());
    for num_terms in [1, 10, 100, 1000, 10_000] {
        let lin: Linear = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 1,
            max_id: 10 * num_terms as u64,
        });
        let state = sample_deterministic(arbitrary_state(lin.used_decision_variable_ids()));
        eval_linear.bench_with_input(
            BenchmarkId::new("eval-linear", num_terms.to_string()),
            &(lin, state),
            |b, (lin, state)| b.iter(|| lin.evaluate(state)),
        );
    }
    eval_linear.finish();

    let mut eval_quadratic = c.benchmark_group("eval-quadratic");
    eval_quadratic.plot_config(plot_config.clone());
    for num_terms in [1, 10, 100, 1000, 10_000] {
        let quad: Quadratic = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 2,
            max_id: 10 * num_terms as u64,
        });
        let state = sample_deterministic(arbitrary_state(quad.used_decision_variable_ids()));
        eval_quadratic.bench_with_input(
            BenchmarkId::new("eval-quadratic", num_terms.to_string()),
            &(quad, state),
            |b, (quad, state)| b.iter(|| quad.evaluate(state)),
        );
    }
    eval_quadratic.finish();
}

criterion_group!(benches, evaluate);
criterion_main!(benches);
