use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};

use ommx::{
    random::{random_deterministic, FunctionParameters},
    v1::Linear,
};

/// Benchmark for squaring a linear function with varying number of terms
fn square_linear(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("square-linear");
    group.plot_config(plot_config.clone());

    // num_terms を変化させた自乗ベンチマーク
    for &num_terms in &[10, 100, 1000] {
        let f: Linear = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 1,
            max_id: 3 * num_terms as u64,
        });
        group.bench_with_input(
            BenchmarkId::new("square-linear", num_terms.to_string()),
            &f,
            |b, f| {
                b.iter(|| {
                    let _ = f.clone() * f.clone();
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, square_linear);
criterion_main!(benches);
