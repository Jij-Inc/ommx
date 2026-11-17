//! Fixed-input regression guardrail for model reference validation (PR #1400).
//!
//! Purpose/regression: detect repeated reference-set construction and checking
//! on successful builder calls, including allocations that leave scaling linear.
//! Origin: https://github.com/Jij-Inc/ommx/pull/1400
//! Boundary: Rust SDK Instance/ParametricInstance builder execution. Fixture
//! construction and cloning are untimed; the full build is timed.
//! Input: 10,000 declared binary variables and 10,000 three-member constraints.
//! Each family uses the same fixed size so comparisons across commits expose
//! per-row overhead without varying unrelated model dimensions.
//! Cost model: variable-table validation plus one reference-existence check per
//! row, structural-kind checks, and active-reference collection. Compare the
//! same family/input on base and head; no exponent-based acceptance criterion.
//! Lifecycle/run policy: persistent guardrail under the existing benchmark
//! workflow. Four fixed cases; no automatic scaling or profiling matrix.
//! Runtime budget: about 5-10 ms per build in local optimized measurements;
//! keep diagnostic parameter sweeps outside this suite.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use ommx::{
    linear, Constraint, DecisionVariable, Equality, Function, IndicatorConstraint,
    IndicatorTriggerValue, Instance, OneHotConstraint, ParameterTable, ParametricInstance, Sense,
    Sos1Constraint, VariableID,
};
use std::collections::BTreeMap;

fn build_references(c: &mut Criterion) {
    const N: u64 = 10_000;
    let variables: BTreeMap<_, _> = (0..N)
        .map(|id| (VariableID::from(id), DecisionVariable::binary()))
        .collect();
    let builder = Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(variables.clone())
        .active_constraints(BTreeMap::new());
    let functions: Vec<Function> = (0..N)
        .map(|id| {
            ((linear!(id) + linear!((id + 1) % N)).unwrap() + linear!((id + 2) % N))
                .unwrap()
                .into()
        })
        .collect();
    let regular = builder.clone().active_constraints(
        (0..N)
            .map(|id| {
                (
                    id.into(),
                    Constraint::equal_to_zero(functions[id as usize].clone()),
                )
            })
            .collect(),
    );
    let indicator = builder.clone().active_indicator_constraints(
        (0..N)
            .map(|id| {
                (
                    id.into(),
                    IndicatorConstraint::new(
                        id.into(),
                        Equality::EqualToZero,
                        functions[id as usize].clone(),
                        IndicatorTriggerValue::One,
                    ),
                )
            })
            .collect(),
    );
    let members = |id: u64| [id.into(), ((id + 1) % N).into(), ((id + 2) % N).into()].into();
    let one_hot = builder.active_one_hot_constraints(
        (0..N)
            .map(|id| (id.into(), OneHotConstraint::new(members(id)).unwrap()))
            .collect(),
    );
    let mut group = c.benchmark_group("variable_reference/build_10000_rows");
    for (family, fixture) in [
        ("regular", regular),
        ("indicator", indicator),
        ("one_hot", one_hot),
    ] {
        group.bench_function(family, |b| {
            b.iter_batched(
                || fixture.clone(),
                |input| black_box(input.build().unwrap()),
                BatchSize::LargeInput,
            );
        });
    }
    let parametric_sos1 = ParametricInstance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(variables)
        .parameters(ParameterTable::default())
        .active_constraints(BTreeMap::new())
        .active_sos1_constraints(
            (0..N)
                .map(|id| (id.into(), Sos1Constraint::new(members(id)).unwrap()))
                .collect(),
        );
    group.bench_function("parametric_sos1", |b| {
        b.iter_batched(
            || parametric_sos1.clone(),
            |input| black_box(input.build().unwrap()),
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

criterion_group!(benches, build_references);
criterion_main!(benches);
