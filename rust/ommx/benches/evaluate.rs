use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use ommx::{
    random::{arbitrary_state, random_deterministic, sample_deterministic, FunctionParameters},
    v1::{Linear, Quadratic},
    Evaluate,
};
use proptest::prelude::Arbitrary;

fn evaluate<T: Evaluate + Arbitrary<Parameters = FunctionParameters>>(
    c: &mut Criterion,
    title: &str,
    max_degree: u32,
) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group(title);
    group.plot_config(plot_config.clone());
    for num_terms in [100, 1000, 10_000] {
        let f: T = random_deterministic(FunctionParameters {
            num_terms,
            max_degree,
            max_id: 10 * num_terms as u64,
        });
        let state = sample_deterministic(arbitrary_state(f.required_ids()));
        group.bench_with_input(
            BenchmarkId::new(title, num_terms.to_string()),
            &(f, state),
            |b, (f, state)| b.iter(|| f.evaluate(state)),
        );
    }
    group.finish();
}

fn evaluate_linear(c: &mut Criterion) {
    evaluate::<Linear>(c, "evaluate-linear", 1);
}

fn evaluate_quadratic(c: &mut Criterion) {
    evaluate::<Quadratic>(c, "evaluate-quadratic", 2);
}

fn evaluate_polynomial(c: &mut Criterion) {
    evaluate::<ommx::v1::Polynomial>(c, "evaluate-polynomial", 3);
}

criterion_group!(
    benches,
    evaluate_linear,
    evaluate_quadratic,
    evaluate_polynomial
);
criterion_main!(benches);
