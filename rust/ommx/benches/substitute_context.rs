//! Diagnostic for the clone-backed generic in-place substitution helpers (#1046).
//! Origin: https://github.com/Jij-Inc/ommx/issues/1046.
//!
//! Purpose/regression: isolate whole-owner atomic staging from semantic rewriting.
//! Boundary: Rust free helpers on Instance and ParametricInstance, not their
//! consuming trait methods and not the owner-specific integer-encoding path.
//! Independent variable: N unrelated NamedFunctions OR removed constraints.
//! Two variables, one active objective, one assignment, and expression sizes stay
//! fixed. No active rows or dependency chains grow with N.
//! Cost model: fixed rewrite plus cloning and disposing the retained table;
//! expect p ~= 1 today. This is a diagnostic hypothesis, not an accepted no-scan
//! contract for generic helpers. Use same-run ratios/exponents; profile if needed
//! to distinguish cloning from necessary work before proposing an optimization.
//! Expected profile: the free helper enters owner Clone, including retained-table
//! allocation/copy, then fixed active rewriting and old-owner/table drop on commit.
//! Input rationale: 256/1024/4096 rows expose scaling without a large full-suite
//! workload. Setup/reset and final fixture disposal are outside measurement;
//! cloning and replacing the old owner INSIDE the free helper remain timed.
//! Lifecycle/run policy: manual diagnostic, included only in the existing full
//! suite (OMMX_BENCHMARK_SUITE=full). Do not add 36 cases to automatic guardrails.
//! Runtime budget: keep each optimized native case below a few milliseconds;
//! account for build-included job cost before promoting a persistent subset.
//!
//! Local run:
//! OMMX_BENCHMARK_SUITE=full cargo bench -p ommx --no-default-features --bench substitute_context

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
    let rhs = Function::from(linear!(1));
    let assignments = AcyclicAssignments::new([(VariableID::from(0), rhs.clone())]).unwrap();
    for helper in ["substitute", "substitute_one", "substitute_acyclic"] {
        let mut group = c.benchmark_group(format!("{family}/{helper}"));
        for count in [256, 1024, 4096] {
            let source = make(count);
            group.bench_with_input(BenchmarkId::from_parameter(count), &source, |b, source| {
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
            });
        }
        group.finish();
    }
}

fn substitute_context(c: &mut Criterion) {
    if std::env::var("OMMX_BENCHMARK_SUITE").as_deref() != Ok("full") {
        return;
    }
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
