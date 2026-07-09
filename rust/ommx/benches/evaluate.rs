//! Persistent scaling guardrails for expression and Instance evaluation.
//!
//! The expression families vary term count and should remain O(N). The
//! Instance families reproduce the evaluate/evaluate_samples boundary from
//! issue #336, using synthetic one-variable constraints so Rust-internal
//! scaling is measured without the heavy MIPLIB profiling fixture.

use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use ommx::{
    linear,
    random::{arbitrary_state, random_deterministic, sample_deterministic},
    Constraint, ConstraintID, DecisionVariable, Evaluate, Function, Instance, Linear,
    LinearParameters, Polynomial, PolynomialParameters, Quadratic, QuadraticParameters, Sampled,
    Sense, VariableID,
};
use proptest::prelude::Arbitrary;

// A 10x span separates linear from quadratic growth without making every
// instrumented expression benchmark pay for a 10,000-term profile.
const EXPRESSION_SCALE: [usize; 3] = [100, 320, 1_000];

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
    for num_terms in EXPRESSION_SCALE {
        let params = parameter_generator(num_terms);
        let f: T = random_deterministic(params);
        let state = sample_deterministic(arbitrary_state(f.required_ids()));
        group.bench_with_input(
            BenchmarkId::new(title, num_terms.to_string()),
            &(f, state),
            |b, (f, state)| b.iter(|| f.evaluate(state, ommx::ATol::default())),
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

fn instance_evaluation_fixture(
    num_constraints: usize,
) -> (Instance, ommx::v1::State, Sampled<ommx::v1::State>) {
    let decision_variables = (0..num_constraints as u64)
        .map(|id| (VariableID::from(id), DecisionVariable::continuous()))
        .collect();
    let constraints = (0..num_constraints as u64)
        .map(|id| {
            (
                ConstraintID::from(id),
                Constraint::equal_to_zero(Function::from(linear!(id))),
            )
        })
        .collect();
    let state: ommx::v1::State = (0..num_constraints as u64).map(|id| (id, 0.0)).collect();
    let samples = Sampled::from(state.clone());
    let instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(constraints)
        .build()
        .unwrap();

    (instance, state, samples)
}

/// Scale one-state Instance evaluation with decision variables and constraints.
fn evaluate_instance_single_state(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("evaluate-instance-single-state");
    group.plot_config(plot_config);
    for num_constraints in [100, 1_000, 10_000] {
        let (instance, state, _) = instance_evaluation_fixture(num_constraints);
        group.bench_with_input(
            BenchmarkId::new(
                "evaluate-instance-single-state",
                num_constraints.to_string(),
            ),
            &(instance, state),
            |b, (instance, state)| {
                b.iter(|| instance.evaluate(state, ommx::ATol::default()).unwrap())
            },
        );
    }
    group.finish();
}

/// Scale single-sample Instance evaluation against the one-state baseline.
fn evaluate_instance_single_sample(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("evaluate-instance-single-sample");
    group.plot_config(plot_config);
    for num_constraints in [100, 1_000, 10_000] {
        let (instance, _, samples) = instance_evaluation_fixture(num_constraints);
        group.bench_with_input(
            BenchmarkId::new(
                "evaluate-instance-single-sample",
                num_constraints.to_string(),
            ),
            &(instance, samples),
            |b, (instance, samples)| {
                b.iter(|| {
                    instance
                        .evaluate_samples(samples, ommx::ATol::default())
                        .unwrap()
                })
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    evaluate_linear,
    evaluate_quadratic,
    evaluate_polynomial,
    evaluate_instance_single_state,
    evaluate_instance_single_sample,
);
criterion_main!(benches);
