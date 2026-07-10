// Benchmark contracts for Instance::log_encode.
//
// Removed-constraint family
// - Purpose: persistent scaling guardrail.
// - Regression: atomic log encoding clones untouched removed state and becomes O(N).
// - Origin: https://github.com/Jij-Inc/ommx/issues/1031 and
//   https://github.com/Jij-Inc/ommx/issues/1045.
// - Measured boundary: Rust SDK Instance::log_encode.
// - Independent variable: removed regular constraint count N; the two decision
//   variables and ten-bit encoding work stay fixed.
// - Cost model/evidence: fixed validation, encoding, and substitution plan;
//   removed rows are not inspected, so the CodSpeed simulation same-run
//   cross-size exponent should be p ~= 0.
//
// Active-constraint family
// - Purpose: persistent scaling guardrail for the necessary active-row scan.
// - Regression: repeated collection rebuilds or nested row scans change the
//   expected O(N) traversal into superlinear work.
// - Origin/boundary: the issues above; Rust SDK operation.
// - Independent variable: active regular constraint count N; variable count,
//   expression size, and ten-bit encoding work stay fixed.
// - Cost model/evidence: inspect N active rows without rewriting them, so p ~= 1.
//   This exponent is a complexity guardrail, not evidence about constant-factor
//   whole-owner cloning; use the removed family or same-input run comparisons
//   for that regression.
//
// Unrelated-named-function family
// - Purpose: persistent fixed-input regression guardrail.
// - Regression: routing through a generic atomic helper clones and rebuilds
//   every unrelated named-function row.
// - Origin: https://github.com/Jij-Inc/ommx/pull/1047.
// - Measured boundary/input: Rust SDK Instance::log_encode with 32,000 fixed,
//   deterministic named functions that do not reference the encoded variable.
// - Cost model/evidence: inspect N functions for affected IDs plus fixed
//   encoding work, without cloning their rows; compare the same URI across
//   commits and use a profile only to diagnose a detected regression.
//
// Unrelated-dependency family
// - Purpose: persistent fixed-input regression guardrail.
// - Regression: atomic log encoding clones every unrelated dependency RHS
//   before adding the encoded variable assignment.
// - Origin: https://github.com/Jij-Inc/ommx/pull/1047#discussion_r3556489103.
// - Measured boundary/input: Rust SDK Instance::log_encode with 8,000 fixed,
//   deterministic dependency assignments whose RHS does not reference the
//   encoded variable.
// - Cost model/evidence: inspect N assignments and rebuild the dependency
//   graph, but clone/rewrite no unrelated RHS; compare the same URI and input
//   shape across commits, using a profile only after a detected regression.
//
// Input rationale: 1,000/8,000/32,000 expose the dominant term while keeping
// the largest measured case below a 25 ms simulation budget. Lifecycle/run
// policy: retain on main; run automatically on main/releases and manually on
// performance PRs through the benchmark workflow.
use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};
use ommx::{
    linear, ATol, AcyclicAssignments, Bound, Constraint, ConstraintID, DecisionVariable, Function,
    Instance, Kind, NamedFunction, NamedFunctionID, RemovedReason, Sense, VariableID,
};
use std::collections::BTreeMap;

/// Width of the integer variable's bound, chosen to require a fixed, small
/// number of auxiliary binary variables regardless of the surrounding
/// instance size.
const INTEGER_BOUND_WIDTH: f64 = 1023.0; // 10 log-encoding bits

/// An instance with a single integer decision variable (id 0) to log-encode,
/// plus `num_removed` unrelated removed regular constraints that log_encode
/// must neither inspect nor rewrite.
fn instance_with_removed_constraints(num_removed: usize) -> Instance {
    let decision_variables = BTreeMap::from([
        (
            VariableID::from(0),
            DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, INTEGER_BOUND_WIDTH).unwrap(),
                ATol::default(),
            )
            .unwrap(),
        ),
        (VariableID::from(1), DecisionVariable::continuous()),
    ]);

    let removed_reason = RemovedReason {
        reason: "ommx.bench.log_encode.removed_regular_constraints".to_string(),
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
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(BTreeMap::new())
        .removed_constraints(removed_constraints)
        .build()
        .unwrap()
}

/// An instance with a single integer decision variable (id 0) to log-encode,
/// plus `num_active` unrelated active regular constraints that don't
/// reference the encoded variable at all, so log_encode inspects but does not
/// rewrite them.
fn instance_with_active_constraints(num_active: usize) -> Instance {
    let decision_variables = BTreeMap::from([
        (
            VariableID::from(0),
            DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, INTEGER_BOUND_WIDTH).unwrap(),
                ATol::default(),
            )
            .unwrap(),
        ),
        (VariableID::from(1), DecisionVariable::continuous()),
    ]);

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
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(constraints)
        .build()
        .unwrap()
}

fn instance_with_unrelated_named_functions(num_named_functions: usize) -> Instance {
    let decision_variables = BTreeMap::from([
        (
            VariableID::from(0),
            DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, INTEGER_BOUND_WIDTH).unwrap(),
                ATol::default(),
            )
            .unwrap(),
        ),
        (VariableID::from(1), DecisionVariable::continuous()),
    ]);
    let named_functions = (0..num_named_functions as u64)
        .map(|id| {
            (
                NamedFunctionID::from(id),
                NamedFunction {
                    function: Function::from(linear!(1)),
                },
            )
        })
        .collect();

    Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(BTreeMap::new())
        .named_functions(named_functions)
        .build()
        .unwrap()
}

fn instance_with_unrelated_dependencies(num_dependencies: usize) -> Instance {
    let mut decision_variables = BTreeMap::from([
        (
            VariableID::from(0),
            DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, INTEGER_BOUND_WIDTH).unwrap(),
                ATol::default(),
            )
            .unwrap(),
        ),
        (VariableID::from(1), DecisionVariable::continuous()),
    ]);
    decision_variables.extend(
        (2..num_dependencies as u64 + 2)
            .map(|id| (VariableID::from(id), DecisionVariable::continuous())),
    );
    let dependencies = AcyclicAssignments::new(
        (2..num_dependencies as u64 + 2)
            .map(|id| (VariableID::from(id), Function::from(linear!(1)))),
    )
    .unwrap();

    Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::Zero)
        .decision_variables(decision_variables)
        .constraints(BTreeMap::new())
        .decision_variable_dependency(dependencies)
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

fn log_encode_unrelated_named_functions(c: &mut Criterion) {
    const NUM_NAMED_FUNCTIONS: usize = 32_000;
    let instance = instance_with_unrelated_named_functions(NUM_NAMED_FUNCTIONS);
    c.bench_function("log-encode-unrelated-named-functions[32000]", |b| {
        b.iter_batched_ref(
            || instance.clone(),
            |instance| {
                instance
                    .log_encode([VariableID::from(0)], ATol::default())
                    .unwrap();
            },
            criterion::BatchSize::LargeInput,
        )
    });
}

fn log_encode_unrelated_dependencies(c: &mut Criterion) {
    const NUM_DEPENDENCIES: usize = 8_000;
    let instance = instance_with_unrelated_dependencies(NUM_DEPENDENCIES);
    c.bench_function("log-encode-unrelated-dependencies[8000]", |b| {
        b.iter_batched_ref(
            || instance.clone(),
            |instance| {
                instance
                    .log_encode([VariableID::from(0)], ATol::default())
                    .unwrap();
            },
            criterion::BatchSize::LargeInput,
        )
    });
}

criterion_group!(
    benches,
    log_encode_removed_constraints,
    log_encode_active_constraints,
    log_encode_unrelated_named_functions,
    log_encode_unrelated_dependencies,
);
criterion_main!(benches);
