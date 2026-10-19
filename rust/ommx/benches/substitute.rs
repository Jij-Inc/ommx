//! Detect repeated full-polynomial scans as the number of assignments grows.
//!
//! Apply 1, 16, or 256 one-to-one variable renamings to a fixed 4096-term
//! quadratic. Renaming preserves the output size, so additional work reflects
//! the cost of handling more assignments rather than polynomial expansion.

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
    let mut group = c.benchmark_group("substitute-variable-renaming");
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
            BenchmarkId::from_parameter(format!("assignments={count}")),
            &assignments,
            |b, assignments| {
                // Keep cloning outside the timed operation.
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
