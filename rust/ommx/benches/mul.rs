use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};

use ommx::{
    random::random_deterministic, Linear, LinearParameters, Polynomial, PolynomialParameters,
    Quadratic, QuadraticParameters,
};

/// Benchmark for squaring a linear function with varying number of terms
fn square_linear(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("square-linear");
    group.plot_config(plot_config.clone());

    for &num_terms in &[10, 100] {
        let f: Linear = random_deterministic(
            LinearParameters::new(num_terms, (3 * num_terms as u64).into()).unwrap(),
        );
        group.bench_with_input(
            BenchmarkId::new("square-linear", num_terms.to_string()),
            &f,
            |b, f| {
                b.iter(|| {
                    let _ = f * f;
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
        let f: Quadratic = random_deterministic(
            QuadraticParameters::new(num_terms, (3 * num_terms as u64).into()).unwrap(),
        );
        group.bench_with_input(
            BenchmarkId::new("square-quadratic", num_terms.to_string()),
            &f,
            |b, f| {
                b.iter(|| {
                    let _ = f * f;
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
        let f: Polynomial = random_deterministic(
            PolynomialParameters::new(num_terms, 3.into(), (3 * num_terms as u64).into()).unwrap(),
        );
        group.bench_with_input(
            BenchmarkId::new("square-polynomial", num_terms.to_string()),
            &f,
            |b, f| {
                b.iter(|| {
                    let _ = f * f;
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, square_linear, square_quadratic, square_polynomial);
criterion_main!(benches);
