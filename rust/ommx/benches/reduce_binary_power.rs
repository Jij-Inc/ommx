// Benchmark contracts for Instance::reduce_binary_power.
//
// Removed-constraint family
// - Purpose: persistent scaling guardrail.
// - Regression: whole-Instance rollback cloning makes untouched removed state O(N).
// - Origin: https://github.com/Jij-Inc/ommx/issues/1031 and
//   https://github.com/Jij-Inc/ommx/issues/1044.
// - Measured boundary: Rust SDK Instance::reduce_binary_power.
// - Independent variable: removed regular constraint count N; all other shape
//   dimensions, including the two decision variables, stay fixed.
// - Cost model/evidence: fixed binary-ID discovery plus one objective rewrite;
//   removed rows are not inspected, so the CodSpeed simulation same-run
//   cross-size exponent should be p ~= 0.
//
// Active-constraint family
// - Purpose: persistent scaling guardrail for the necessary active-row scan.
// - Regression: an extra owner clone adds avoidable work on top of that scan.
// - Origin/boundary: the issues above; Rust SDK operation.
// - Independent variable: active regular constraint count N; variable count,
//   expression size, and the single objective rewrite stay fixed.
// - Cost model/evidence: inspect N active rows without rewriting them, so p ~= 1.
//
// Input rationale: 1,000/8,000/32,000 expose the dominant term while keeping
// the largest measured case below a 25 ms simulation budget. Lifecycle/run
// policy: retain on main; run automatically on main/releases and manually on
// performance PRs through the benchmark workflow.
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
/// `reduce_binary_power` must neither inspect nor rewrite.
fn instance_with_removed_constraints(num_removed: usize) -> Instance {
    let decision_variables = BTreeMap::from([
        (VariableID::from(0), DecisionVariable::binary()),
        (VariableID::from(1), DecisionVariable::continuous()),
    ]);

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
                    Constraint::equal_to_zero(Function::from(linear!(1))),
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
/// don't reference `x0` at all, so `reduce_binary_power` inspects but does not
/// rewrite them.
fn instance_with_active_constraints(num_active: usize) -> Instance {
    let decision_variables = BTreeMap::from([
        (VariableID::from(0), DecisionVariable::binary()),
        (VariableID::from(1), DecisionVariable::continuous()),
    ]);

    let objective = Function::Quadratic(quadratic!(0, 0).into());

    let constraints = (1..=num_active as u64)
        .map(|id| {
            (
                ConstraintID::from(id),
                Constraint::equal_to_zero(Function::from(linear!(1))),
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
