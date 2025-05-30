// Create partial evaluation benchmarks for Linear
use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use ommx::{
    random::{arbitrary_state, random_deterministic, sample_deterministic},
    Evaluate, Linear, LinearParameters, Polynomial, PolynomialParameters, Quadratic,
    QuadraticParameters, VariableID, VariableIDSet,
};
use proptest::prelude::Arbitrary;

fn bench_partial_evaluate<T, Parameters>(
    c: &mut Criterion,
    group_name: &str,
    id_selector: impl Fn(VariableIDSet) -> VariableIDSet,
    parameter_generator: impl Fn(usize) -> Parameters,
) where
    T: Evaluate + Clone + Arbitrary<Parameters = Parameters>,
{
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group(group_name);
    group.plot_config(plot_config.clone());
    for num_terms in [100, 1_000, 10_000] {
        let lin: T = random_deterministic(parameter_generator(num_terms));
        let ids = id_selector(lin.required_ids());
        let state = sample_deterministic(arbitrary_state(ids));
        group.bench_with_input(
            BenchmarkId::new(group_name, num_terms.to_string()),
            &(lin, state),
            |b, (lin, state)| {
                b.iter_batched_ref(
                    || lin.clone(),
                    |f| {
                        f.partial_evaluate(state, ommx::ATol::default()).unwrap();
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn all_ids(ids: VariableIDSet) -> VariableIDSet {
    ids
}

fn half_ids(ids: VariableIDSet) -> VariableIDSet {
    let n = ids.len() / 2;
    ids.into_iter().take(n).collect()
}

fn one_id(ids: VariableIDSet) -> VariableIDSet {
    ids.into_iter().take(1).collect()
}

/// Substitute all decision variables in a linear function
fn partial_evaluate_linear_all(c: &mut Criterion) {
    bench_partial_evaluate::<Linear, _>(c, "partial-evaluate-linear-all", all_ids, |num_terms| {
        LinearParameters::new(num_terms, VariableID::from(10 * num_terms as u64)).unwrap()
    });
}

/// Substitute half of the decision variables in a linear function
fn partial_evaluate_linear_half(c: &mut Criterion) {
    bench_partial_evaluate::<Linear, _>(c, "partial-evaluate-linear-half", half_ids, |num_terms| {
        LinearParameters::new(num_terms, VariableID::from(10 * num_terms as u64)).unwrap()
    });
}

/// Substitute one decision variable in a linear function
fn partial_evaluate_linear_one(c: &mut Criterion) {
    bench_partial_evaluate::<Linear, _>(c, "partial-evaluate-linear-one", one_id, |num_terms| {
        LinearParameters::new(num_terms, VariableID::from(10 * num_terms as u64)).unwrap()
    });
}

/// Substitute all decision variables in a quadratic function
fn partial_evaluate_quadratic_all(c: &mut Criterion) {
    bench_partial_evaluate::<Quadratic, _>(
        c,
        "partial-evaluate-quadratic-all",
        all_ids,
        |num_terms| {
            QuadraticParameters::new(num_terms, VariableID::from(10 * num_terms as u64)).unwrap()
        },
    );
}

/// Substitute half of the decision variables in a quadratic function
fn partial_evaluate_quadratic_half(c: &mut Criterion) {
    bench_partial_evaluate::<Quadratic, _>(
        c,
        "partial-evaluate-quadratic-half",
        half_ids,
        |num_terms| {
            QuadraticParameters::new(num_terms, VariableID::from(10 * num_terms as u64)).unwrap()
        },
    );
}

/// Substitute one decision variable in a quadratic function
fn partial_evaluate_quadratic_one(c: &mut Criterion) {
    bench_partial_evaluate::<Quadratic, _>(
        c,
        "partial-evaluate-quadratic-one",
        one_id,
        |num_terms| {
            QuadraticParameters::new(num_terms, VariableID::from(10 * num_terms as u64)).unwrap()
        },
    );
}

/// Substitute all decision variables in a polynomial function
fn partial_evaluate_polynomial_all(c: &mut Criterion) {
    bench_partial_evaluate::<Polynomial, _>(
        c,
        "partial-evaluate-polynomial-all",
        all_ids,
        |num_terms| {
            PolynomialParameters::new(num_terms, 5.into(), VariableID::from(10 * num_terms as u64))
                .unwrap()
        },
    );
}

/// Substitute half of the decision variables in a polynomial function
fn partial_evaluate_polynomial_half(c: &mut Criterion) {
    bench_partial_evaluate::<Polynomial, _>(
        c,
        "partial-evaluate-polynomial-half",
        half_ids,
        |num_terms| {
            PolynomialParameters::new(num_terms, 5.into(), VariableID::from(10 * num_terms as u64))
                .unwrap()
        },
    );
}

/// Substitute one decision variable in a polynomial function
fn partial_evaluate_polynomial_one(c: &mut Criterion) {
    bench_partial_evaluate::<Polynomial, _>(
        c,
        "partial-evaluate-polynomial-one",
        one_id,
        |num_terms| {
            PolynomialParameters::new(num_terms, 5.into(), VariableID::from(10 * num_terms as u64))
                .unwrap()
        },
    );
}

criterion_group!(
    benches,
    partial_evaluate_linear_all,
    partial_evaluate_linear_half,
    partial_evaluate_linear_one,
    partial_evaluate_quadratic_all,
    partial_evaluate_quadratic_half,
    partial_evaluate_quadratic_one,
    partial_evaluate_polynomial_all,
    partial_evaluate_polynomial_half,
    partial_evaluate_polynomial_one,
);
criterion_main!(benches);
