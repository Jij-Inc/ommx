use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};

use ommx::{
    random::{random_deterministic, FunctionParameters},
    v1::{Linear, Polynomial, Quadratic},
};

/// Benchmark for squaring a linear function with varying number of terms
fn square_linear(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("square-linear");
    group.plot_config(plot_config.clone());

    for &num_terms in &[10, 100] {
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

/// Benchmark for squaring a quadratic function with varying number of terms
fn square_quadratic(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("square-quadratic");
    group.plot_config(plot_config.clone());

    for &num_terms in &[10, 100] {
        let f: Quadratic = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 2,
            max_id: 3 * num_terms as u64,
        });
        group.bench_with_input(
            BenchmarkId::new("square-quadratic", num_terms.to_string()),
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

/// Benchmark for squaring a polynomial function with varying number of terms
fn square_polynomial(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("square-polynomial");
    group.plot_config(plot_config.clone());

    for &num_terms in &[10, 100] {
        let f: Polynomial = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 3,
            max_id: 3 * num_terms as u64,
        });
        group.bench_with_input(
            BenchmarkId::new("square-polynomial", num_terms.to_string()),
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

criterion_group!(benches, square_linear, square_quadratic, square_polynomial);
criterion_main!(benches);
