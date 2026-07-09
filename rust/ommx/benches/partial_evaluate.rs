//! Persistent scaling guardrails for partial evaluation.
//!
//! Expression families vary total term count while assignment density is one,
//! half, or all required IDs; each should traverse terms linearly. Instance
//! families originate from issue #1027 and compare removed-only transaction
//! overhead with active-constraint atomic and consuming paths.

use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use ommx::{
    linear,
    random::{arbitrary_state, random_deterministic, sample_deterministic},
    Constraint, ConstraintID, DecisionVariable, Evaluate, Function, Instance, Linear,
    LinearParameters, Polynomial, PolynomialParameters, Quadratic, QuadraticParameters,
    RemovedReason, Sense, VariableID, VariableIDSet,
};
use proptest::prelude::Arbitrary;
use std::collections::BTreeMap;

fn removed_constraint_instance(num_constraints: usize) -> (Instance, ommx::v1::State) {
    let decision_variables = (0..num_constraints as u64)
        .map(|id| (VariableID::from(id), DecisionVariable::continuous()))
        .collect();
    let removed_reason = RemovedReason {
        reason: "ommx.bench.partial_evaluate.removed_regular_constraints".to_string(),
        parameters: Default::default(),
    };
    let removed_constraints = (0..num_constraints as u64)
        .map(|id| {
            (
                ConstraintID::from(id),
                (
                    Constraint::equal_to_zero(Function::from(linear!(id))),
                    removed_reason.clone(),
                ),
            )
        })
        .collect();
    let state = (0..num_constraints as u64).map(|id| (id, 0.0)).collect();
    let instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(BTreeMap::new())
        .removed_constraints(removed_constraints)
        .build()
        .unwrap();

    (instance, state)
}

fn active_constraint_instance(num_constraints: usize) -> (Instance, ommx::v1::State) {
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
    let state = (0..num_constraints as u64).map(|id| (id, 0.0)).collect();
    let instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(constraints)
        .build()
        .unwrap();

    (instance, state)
}

/// Substitute all decision variables in an Instance that has many regular
/// constraints already moved into removed_constraints.
fn partial_evaluate_instance_removed_constraints(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("partial-evaluate-instance-removed-constraints");
    group.plot_config(plot_config);

    for num_constraints in [1_000, 8_000, 32_000] {
        let (instance, state) = removed_constraint_instance(num_constraints);
        group.bench_with_input(
            BenchmarkId::new(
                "partial-evaluate-instance-removed-constraints",
                num_constraints.to_string(),
            ),
            &(instance, state),
            |b, (instance, state)| {
                b.iter_batched_ref(
                    || instance.clone(),
                    |instance| {
                        instance
                            .partial_evaluate(state, ommx::ATol::default())
                            .unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

/// Substitute all decision variables in an Instance that still has many active
/// regular constraints. This exercises the fallback path where borrowed
/// partial_evaluate keeps atomic rollback by cloning the whole Instance, while
/// into_partial_evaluated can mutate the consumed Instance directly.
fn partial_evaluate_instance_active_constraints(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("partial-evaluate-instance-active-constraints");
    group.plot_config(plot_config);

    for num_constraints in [1_000, 8_000, 32_000] {
        let (instance, state) = active_constraint_instance(num_constraints);
        group.bench_with_input(
            BenchmarkId::new(
                "partial-evaluate-instance-active-constraints-atomic",
                num_constraints.to_string(),
            ),
            &(instance.clone(), state.clone()),
            |b, (instance, state)| {
                b.iter_batched_ref(
                    || instance.clone(),
                    |instance| {
                        instance
                            .partial_evaluate(state, ommx::ATol::default())
                            .unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
        group.bench_with_input(
            BenchmarkId::new(
                "into-partial-evaluated-instance-active-constraints-in-place",
                num_constraints.to_string(),
            ),
            &(instance, state),
            |b, (instance, state)| {
                b.iter_batched(
                    || instance.clone(),
                    |instance| {
                        instance
                            .into_partial_evaluated(state, ommx::ATol::default())
                            .unwrap()
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

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
    partial_evaluate_instance_removed_constraints,
    partial_evaluate_instance_active_constraints,
);
criterion_main!(benches);
