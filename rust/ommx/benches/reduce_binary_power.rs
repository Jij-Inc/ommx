// Benchmark Instance::reduce_binary_power's clone-backed rollback overhead.
//
// reduce_binary_power clones the whole Instance before rewriting the
// objective and active regular constraints, even though only a handful of
// binary-power terms actually change. These benchmarks hold the amount of
// real rewrite work constant (one binary-squared term in the objective)
// while growing unrelated instance state, to show how much of the cost is
// attributable to the whole-instance clone rather than the rewrite itself.
use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use ommx::{
    linear, quadratic, Constraint, ConstraintID, DecisionVariable, Function, Instance,
    RemovedReason, Sense, VariableID,
};
use std::collections::BTreeMap;

/// An instance whose objective has a single binary-power term to reduce
/// (`x0^2`), plus `num_removed` unrelated removed regular constraints that
/// `reduce_binary_power` must still pay to clone but never rewrites.
fn instance_with_removed_constraints(num_removed: usize) -> Instance {
    let mut decision_variables = BTreeMap::new();
    decision_variables.insert(VariableID::from(0), DecisionVariable::binary());
    for id in 1..=num_removed as u64 {
        decision_variables.insert(VariableID::from(id), DecisionVariable::continuous());
    }

    let objective = Function::Quadratic(quadratic!(0, 0).into());

    let removed_reason = RemovedReason {
        reason: "ommx.bench.reduce_binary_power.removed_regular_constraints".to_string(),
        parameters: Default::default(),
    };
    let removed_constraints = (1..=num_removed as u64)
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

    Instance::builder()
        .sense(Sense::Minimize)
        .objective(objective)
        .decision_variables(decision_variables)
        .constraints(BTreeMap::new())
        .removed_constraints(removed_constraints)
        .build()
        .unwrap()
}

/// An instance whose objective has a single binary-power term to reduce
/// (`x0^2`), plus `num_active` unrelated active regular constraints that
/// don't reference `x0` at all, so `reduce_binary_power` clones them but
/// leaves every one of them untouched.
fn instance_with_active_constraints(num_active: usize) -> Instance {
    let mut decision_variables = BTreeMap::new();
    decision_variables.insert(VariableID::from(0), DecisionVariable::binary());
    for id in 1..=num_active as u64 {
        decision_variables.insert(VariableID::from(id), DecisionVariable::continuous());
    }

    let objective = Function::Quadratic(quadratic!(0, 0).into());

    let constraints = (1..=num_active as u64)
        .map(|id| {
            (
                ConstraintID::from(id),
                Constraint::equal_to_zero(Function::from(linear!(id))),
            )
        })
        .collect();

    Instance::builder()
        .sense(Sense::Minimize)
        .objective(objective)
        .decision_variables(decision_variables)
        .constraints(constraints)
        .build()
        .unwrap()
}

fn reduce_binary_power_removed_constraints(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("reduce-binary-power-removed-constraints");
    group.plot_config(plot_config);

    for num_removed in [1_000, 8_000, 32_000] {
        let instance = instance_with_removed_constraints(num_removed);
        group.bench_with_input(
            BenchmarkId::new(
                "reduce-binary-power-removed-constraints",
                num_removed.to_string(),
            ),
            &instance,
            |b, instance| {
                b.iter_batched_ref(
                    || instance.clone(),
                    |instance| {
                        instance.reduce_binary_power().unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

fn reduce_binary_power_active_constraints(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("reduce-binary-power-active-constraints");
    group.plot_config(plot_config);

    for num_active in [1_000, 8_000, 32_000] {
        let instance = instance_with_active_constraints(num_active);
        group.bench_with_input(
            BenchmarkId::new(
                "reduce-binary-power-active-constraints",
                num_active.to_string(),
            ),
            &instance,
            |b, instance| {
                b.iter_batched_ref(
                    || instance.clone(),
                    |instance| {
                        instance.reduce_binary_power().unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    reduce_binary_power_removed_constraints,
    reduce_binary_power_active_constraints,
);
criterion_main!(benches);
