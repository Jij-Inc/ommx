// Benchmark Instance::log_encode's clone-backed rollback overhead.
//
// log_encode clones the whole Instance once, encodes the requested integer
// variable(s) on the clone, then commits the clone back on success. These
// benchmarks hold the amount of real encoding work constant (one integer
// variable log-encoded into a fixed number of bits) while growing unrelated
// instance state, to show how much of the cost is attributable to the
// whole-instance clone rather than the encoding itself.
use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use ommx::{
    linear, ATol, Bound, Constraint, ConstraintID, DecisionVariable, Function, Instance, Kind,
    RemovedReason, Sense, VariableID,
};
use std::collections::BTreeMap;

/// Width of the integer variable's bound, chosen to require a fixed, small
/// number of auxiliary binary variables regardless of the surrounding
/// instance size.
const INTEGER_BOUND_WIDTH: f64 = 1023.0; // 10 log-encoding bits

/// An instance with a single integer decision variable (id 0) to log-encode,
/// plus `num_removed` unrelated removed regular constraints that log_encode
/// must still pay to clone but never touches.
fn instance_with_removed_constraints(num_removed: usize) -> Instance {
    let mut decision_variables = BTreeMap::new();
    decision_variables.insert(
        VariableID::from(0),
        DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, INTEGER_BOUND_WIDTH).unwrap(),
            ATol::default(),
        )
        .unwrap(),
    );
    for id in 1..=num_removed as u64 {
        decision_variables.insert(VariableID::from(id), DecisionVariable::continuous());
    }

    let removed_reason = RemovedReason {
        reason: "ommx.bench.log_encode.removed_regular_constraints".to_string(),
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
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(BTreeMap::new())
        .removed_constraints(removed_constraints)
        .build()
        .unwrap()
}

/// An instance with a single integer decision variable (id 0) to log-encode,
/// plus `num_active` unrelated active regular constraints that don't
/// reference the encoded variable at all, so log_encode clones them but
/// leaves every one of them untouched.
fn instance_with_active_constraints(num_active: usize) -> Instance {
    let mut decision_variables = BTreeMap::new();
    decision_variables.insert(
        VariableID::from(0),
        DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, INTEGER_BOUND_WIDTH).unwrap(),
            ATol::default(),
        )
        .unwrap(),
    );
    for id in 1..=num_active as u64 {
        decision_variables.insert(VariableID::from(id), DecisionVariable::continuous());
    }

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
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(constraints)
        .build()
        .unwrap()
}

fn log_encode_removed_constraints(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("log-encode-removed-constraints");
    group.plot_config(plot_config);

    for num_removed in [1_000, 8_000, 32_000] {
        let instance = instance_with_removed_constraints(num_removed);
        group.bench_with_input(
            BenchmarkId::new("log-encode-removed-constraints", num_removed.to_string()),
            &instance,
            |b, instance| {
                b.iter_batched_ref(
                    || instance.clone(),
                    |instance| {
                        instance
                            .log_encode([VariableID::from(0)], ATol::default())
                            .unwrap();
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

fn log_encode_active_constraints(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("log-encode-active-constraints");
    group.plot_config(plot_config);

    for num_active in [1_000, 8_000, 32_000] {
        let instance = instance_with_active_constraints(num_active);
        group.bench_with_input(
            BenchmarkId::new("log-encode-active-constraints", num_active.to_string()),
            &instance,
            |b, instance| {
                b.iter_batched_ref(
                    || instance.clone(),
                    |instance| {
                        instance
                            .log_encode([VariableID::from(0)], ATol::default())
                            .unwrap();
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
    log_encode_removed_constraints,
    log_encode_active_constraints,
);
criterion_main!(benches);
