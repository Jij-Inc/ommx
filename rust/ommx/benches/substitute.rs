// Purpose: persistent scaling guardrail for batched substitution.
// Regression: scanning and rebuilding the full polynomial for each assignment
// makes a K-variable rewrite cost O(M*K) instead of O(M*degree + K).
// Origin: https://github.com/Jij-Inc/ommx/pull/1253
// Measured boundary: Rust Function::substitute_acyclic; fixture cloning and
// assignment construction are excluded from timing.
// Independent variable: K = 1, 16, 256 independent variable renamings.
// Fixed shape: M = 4096 quadratic terms on 256 variables, one-to-one renamings,
// so input/output term count and degree stay fixed even as K increases.
// Expected evidence: a same-run cross-size table with exponent near zero,
// rather than the old exponent near one. Small variations reflect RHS lookup.
// Input rationale: enough terms to expose repeated scans without term expansion.
// Lifecycle/run policy: retain in the Rust suite; use the existing benchmark
// workflow policy and manual native timings during performance investigations.
// Runtime budget: three small cases, under 10 ms per optimized invocation.
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use ommx::{linear, AcyclicAssignments, Coefficient, Function, Quadratic, Substitute, VariableID};

fn substitute(c: &mut Criterion) {
    let pairs = (0_u64..256).flat_map(|id| (1..=16).map(move |offset| (id, (id + offset) % 256)));
    let (rows, columns): (Vec<_>, Vec<_>) = pairs
        .map(|(row, column)| (VariableID::from(row), VariableID::from(column)))
        .unzip();
    let quadratic = Quadratic::from_coo(rows, columns, vec![Coefficient::one(); 4096]).unwrap();
    assert_eq!(quadratic.num_terms(), 4096);
    let function = Function::from(quadratic);
    let mut group = c.benchmark_group("substitute-independent-fixed-terms");
    for count in [1_u64, 16, 256] {
        let assignments = AcyclicAssignments::new(
            (0..count).map(|id| (id.into(), Function::from(linear!(512 + id)))),
        )
        .unwrap();
        assert_eq!(
            function
                .clone()
                .substitute_acyclic(&assignments)
                .unwrap()
                .num_terms(),
            Some(4096),
        );
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &assignments,
            |b, assignments| {
                b.iter_batched(
                    || function.clone(),
                    |function| function.substitute_acyclic(assignments).unwrap(),
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, substitute);
criterion_main!(benches);
