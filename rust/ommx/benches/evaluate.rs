use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use ommx::{
    random::{arbitrary_state, random_deterministic, sample_deterministic},
    Evaluate, Linear, LinearParameters, Polynomial, PolynomialParameters, Quadratic,
    QuadraticParameters, VariableID,
};
use proptest::prelude::Arbitrary;

fn evaluate<T, Params>(
    c: &mut Criterion,
    title: &str,
    parameter_generator: impl Fn(usize) -> Params,
) where
    T: Evaluate + Clone + Arbitrary<Parameters = Params>,
{
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group(title);
    group.plot_config(plot_config.clone());
    for num_terms in [100, 1000, 10_000] {
        let params = parameter_generator(num_terms);
        let f: T = random_deterministic(params);
        let state = sample_deterministic(arbitrary_state(f.required_ids()));
        group.bench_with_input(
            BenchmarkId::new(title, num_terms.to_string()),
            &(f, state),
            |b, (f, state)| b.iter(|| f.evaluate(state, 1e-9)),
        );
    }
    group.finish();
}

fn evaluate_linear(c: &mut Criterion) {
    evaluate::<Linear, LinearParameters>(c, "evaluate-linear", |num_terms| {
        LinearParameters::new(num_terms, VariableID::from(10 * num_terms as u64)).unwrap()
    });
}

fn evaluate_quadratic(c: &mut Criterion) {
    evaluate::<Quadratic, QuadraticParameters>(c, "evaluate-quadratic", |num_terms| {
        QuadraticParameters::new(num_terms, VariableID::from(10 * num_terms as u64)).unwrap()
    });
}

fn evaluate_polynomial(c: &mut Criterion) {
    evaluate::<Polynomial, PolynomialParameters>(c, "evaluate-polynomial", |num_terms| {
        PolynomialParameters::new(num_terms, 3.into(), VariableID::from(10 * num_terms as u64))
            .unwrap()
    });
}

criterion_group!(
    benches,
    evaluate_linear,
    evaluate_quadratic,
    evaluate_polynomial
);
criterion_main!(benches);
