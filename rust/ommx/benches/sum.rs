use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};

use num::Zero;
use ommx::{
    random::{random_deterministic, FunctionParameters},
    v1::Linear,
};

/// Benchmark for summation of many linear functions with three terms
fn sum_linear_small_many(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("sum-linear-small-many");
    group.plot_config(plot_config.clone());
    for num_functions in [10, 100, 1000, 10_000, 100_000] {
        let functions = (0..num_functions)
            .map(|_| -> Linear {
                random_deterministic(FunctionParameters {
                    num_terms: 3,
                    max_degree: 1,
                    max_id: num_functions,
                })
            })
            .collect::<Vec<_>>();
        group.bench_with_input(
            BenchmarkId::new("sum-linear-small-many", num_functions.to_string()),
            &functions,
            |b, linears| {
                b.iter(|| {
                    linears
                        .iter()
                        .fold(Linear::zero(), |acc, lin| acc + lin.clone())
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, sum_linear_small_many);
criterion_main!(benches);
