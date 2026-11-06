//! PR regression guardrails for the generic in-place substitution helpers (#1046).
//! Origin: https://github.com/Jij-Inc/ommx/issues/1046.
//!
//! Purpose/regression: detect additional retained-table scans, clones, or rebuilds
//! in each public helper by comparing the same fixed input across commits.
//! Boundary: Rust free helpers on Instance and ParametricInstance, not their
//! consuming trait methods and not the owner-specific integer-encoding path.
//! Input: 4,096 unrelated NamedFunctions OR removed constraints, measured separately.
//! Two variables, one active objective, one assignment, and expression sizes stay
//! fixed. Both roots and all three helpers have distinct entry paths, so retain
//! one input per root/helper/table combination rather than three sizes of each.
//! Cost model: fixed rewrite plus the helper's existing owner clone and disposal.
//! This baseline includes retained-context copying; it is not the no-traversal
//! contract of integer encoding. A PR regression matters even if its exponent
//! stays linear. Cross-size analysis and profiles are manual follow-up as needed.
//! Expected profile: the free helper enters owner Clone, including retained-table
//! allocation/copy, then fixed active rewriting and old-owner/table drop on commit.
//! Input rationale: 4,096 rows expose redundant table work with bounded case cost.
//! Setup/reset and final fixture disposal are outside measurement;
//! cloning and replacing the old owner INSIDE the free helper remain timed.
//! Lifecycle/run policy: 12 persistent guardrails in the existing Rust suite,
//! compared on ready PRs, main, and releases without an opt-in environment flag.
//! Runtime budget: keep each optimized native case below a few milliseconds;
//! account for build-included job cost before adding more cases.
//!
//! Local run:
//! cargo bench -p ommx --no-default-features --bench substitute_context

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use ommx::{
    linear, AcyclicAssignments, Constraint, ConstraintID, DecisionVariable, Function, Instance,
    NamedFunction, NamedFunctionID, ParametricInstance, RemovedReason, Sense, Substitute,
    VariableID,
};
use std::collections::BTreeMap;

fn instance(count: usize, named_functions: bool) -> Instance {
    let mut names = BTreeMap::new();
    let mut removed = BTreeMap::new();
    for id in 0..count as u64 {
        if named_functions {
            names.insert(
                NamedFunctionID::from(id),
                NamedFunction {
                    function: Function::from(linear!(1)),
                },
            );
        } else {
            removed.insert(
                ConstraintID::from(id),
                (
                    Constraint::equal_to_zero(Function::from(linear!(1))),
                    RemovedReason {
                        reason: "ommx.bench.substitute.context".into(),
                        parameters: Default::default(),
                    },
                ),
            );
        }
    }
    Instance::builder()
        .sense(Sense::Minimize)
        .objective(Function::from(linear!(0)))
        .decision_variables(BTreeMap::from([
            (VariableID::from(0), DecisionVariable::continuous()),
            (VariableID::from(1), DecisionVariable::continuous()),
        ]))
        .constraints(BTreeMap::new())
        .removed_constraints(removed)
        .named_functions(names)
        .build()
        .unwrap()
}

fn benchmark_owner<T: Substitute<Output = T> + Clone>(
    c: &mut Criterion,
    family: &str,
    make: impl Fn(usize) -> T,
) {
    const NUM_CONTEXT_ROWS: usize = 4096;
    let source = make(NUM_CONTEXT_ROWS);
    let rhs = Function::from(linear!(1));
    let assignments = AcyclicAssignments::new([(VariableID::from(0), rhs.clone())]).unwrap();
    for helper in ["substitute", "substitute_one", "substitute_acyclic"] {
        let mut group = c.benchmark_group(format!("{family}/{helper}"));
        group.bench_with_input(
            BenchmarkId::from_parameter(NUM_CONTEXT_ROWS),
            &source,
            |b, source| {
                b.iter_batched_ref(
                    || source.clone(),
                    |target| match helper {
                        "substitute" => {
                            ommx::substitute(target, [(VariableID::from(0), rhs.clone())]).unwrap()
                        }
                        "substitute_one" => {
                            ommx::substitute_one(target, VariableID::from(0), &rhs).unwrap()
                        }
                        "substitute_acyclic" => {
                            ommx::substitute_acyclic(target, &assignments).unwrap()
                        }
                        _ => unreachable!(),
                    },
                    BatchSize::LargeInput,
                );
            },
        );
        group.finish();
    }
}

fn substitute_context(c: &mut Criterion) {
    for (context, named) in [("named-functions", true), ("removed-constraints", false)] {
        benchmark_owner(c, &format!("substitute-context/instance/{context}"), |n| {
            instance(n, named)
        });
        benchmark_owner(
            c,
            &format!("substitute-context/parametric/{context}"),
            |n| ParametricInstance::from(instance(n, named)),
        );
    }
}

criterion_group!(benches, substitute_context);
criterion_main!(benches);
