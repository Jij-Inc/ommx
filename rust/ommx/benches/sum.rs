use criterion::{
    criterion_group, criterion_main, AxisScale, BenchmarkId, Criterion, PlotConfiguration,
};

use num::Zero;
use ommx::{
    random::{random_deterministic, FunctionParameters},
    v1::{Linear, Polynomial, Quadratic},
};

/// Benchmark for summation of many linear functions with three terms
fn sum_linear_small_many(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("sum-linear-small-many");
    group.plot_config(plot_config.clone());
    for num_functions in [10, 100, 1000, 10_000] {
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

fn sum_linear_large_little(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("sum-linear-large-little");
    group.plot_config(plot_config.clone());
    for num_terms in [10, 100, 1000, 10_000] {
        let functions = (0..3)
            .map(|_| -> Linear {
                random_deterministic(FunctionParameters {
                    num_terms,
                    max_degree: 1,
                    max_id: 3 * num_terms as u64,
                })
            })
            .collect::<Vec<_>>();
        group.bench_with_input(
            BenchmarkId::new("sum-linear-large-little", num_terms.to_string()),
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

/// Benchmark for summation of many quadratic functions with three terms
fn sum_quadratic_small_many(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("sum-quadratic-small-many");
    group.plot_config(plot_config.clone());
    for num_functions in [10, 100, 1000, 10_000] {
        let functions = (0..num_functions)
            .map(|_| -> Quadratic {
                random_deterministic(FunctionParameters {
                    num_terms: 3,
                    max_degree: 2,
                    max_id: num_functions as u64,
                })
            })
            .collect::<Vec<_>>();
        group.bench_with_input(
            BenchmarkId::new("sum-quadratic-small-many", num_functions.to_string()),
            &functions,
            |b, quads| {
                b.iter(|| {
                    quads
                        .iter()
                        .fold(Quadratic::zero(), |acc, q| acc + q.clone())
                })
            },
        );
    }
    group.finish();
}

/// Benchmark for summation of few quadratic functions with many terms
fn sum_quadratic_large_little(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("sum-quadratic-large-little");
    group.plot_config(plot_config.clone());
    for num_terms in [10, 100, 1000, 10_000] {
        let functions = (0..3)
            .map(|_| -> Quadratic {
                random_deterministic(FunctionParameters {
                    num_terms,
                    max_degree: 2,
                    max_id: (3 * num_terms) as u64,
                })
            })
            .collect::<Vec<_>>();
        group.bench_with_input(
            BenchmarkId::new("sum-quadratic-large-little", num_terms.to_string()),
            &functions,
            |b, quads| {
                b.iter(|| {
                    quads
                        .iter()
                        .fold(Quadratic::zero(), |acc, q| acc + q.clone())
                })
            },
        );
    }
    group.finish();
}

/// Benchmark for summation of many polynomial functions with three terms
fn sum_polynomial_small_many(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("sum-polynomial-small-many");
    group.plot_config(plot_config.clone());
    for num_functions in [10, 100, 1000, 10_000] {
        let functions = (0..num_functions)
            .map(|_| -> Polynomial {
                random_deterministic(FunctionParameters {
                    num_terms: 3,
                    max_degree: 3,
                    max_id: num_functions as u64,
                })
            })
            .collect::<Vec<_>>();
        group.bench_with_input(
            BenchmarkId::new("sum-polynomial-small-many", num_functions.to_string()),
            &functions,
            |b, polys| {
                b.iter(|| {
                    polys
                        .iter()
                        .fold(Polynomial::zero(), |acc, p| acc + p.clone())
                })
            },
        );
    }
    group.finish();
}

/// Benchmark for summation of few polynomial functions with many terms
fn sum_polynomial_large_little(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("sum-polynomial-large-little");
    group.plot_config(plot_config.clone());
    for num_terms in [10, 100, 1000, 10_000] {
        let functions = (0..3)
            .map(|_| -> Polynomial {
                random_deterministic(FunctionParameters {
                    num_terms,
                    max_degree: 3,
                    max_id: (3 * num_terms) as u64,
                })
            })
            .collect::<Vec<_>>();
        group.bench_with_input(
            BenchmarkId::new("sum-polynomial-large-little", num_terms.to_string()),
            &functions,
            |b, polys| {
                b.iter(|| {
                    polys
                        .iter()
                        .fold(Polynomial::zero(), |acc, p| acc + p.clone())
                })
            },
        );
    }
    group.finish();
}

/// Benchmark for summation of linear + quadratic functions with varying terms
fn add_linear_quadratic_large(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("add-linear-quadratic");
    group.plot_config(plot_config.clone());
    for num_terms in [10, 100, 1000, 10_000] {
        let lin: Linear = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 1,
            max_id: (3 * num_terms) as u64,
        });
        let quad: Quadratic = random_deterministic(FunctionParameters {
            num_terms,
            max_degree: 2,
            max_id: (3 * num_terms) as u64,
        });
        group.bench_with_input(
            BenchmarkId::new("add-linear-quadratic", num_terms.to_string()),
            &(lin, quad),
            |b, (lin, quad)| b.iter(|| lin.clone() + quad.clone()),
        );
    }
    group.finish();
}

fn add_small_many_linear_to_quadratic(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("add-small-many-linear-to-quadratic");
    group.plot_config(plot_config.clone());
    for num_lin in [10, 100, 1000, 10_000] {
        let lins = (0..num_lin)
            .map(|_| -> Linear {
                random_deterministic(FunctionParameters {
                    num_terms: 3,
                    max_degree: 1,
                    max_id: 3 * num_lin as u64,
                })
            })
            .collect::<Vec<_>>();
        let quad: Quadratic = random_deterministic(FunctionParameters {
            num_terms: 3,
            max_degree: 2,
            max_id: (3 * num_lin) as u64,
        });
        group.bench_with_input(
            BenchmarkId::new("add-small-many-linear-to-quadratic", num_lin.to_string()),
            &(lins, quad),
            |b, (lins, quad)| {
                b.iter(|| lins.iter().fold(quad.clone(), |acc, lin| acc + lin.clone()))
            },
        );
    }
    group.finish();
}

fn add_small_many_linear_to_polynomial(c: &mut Criterion) {
    let plot_config = PlotConfiguration::default().summary_scale(AxisScale::Logarithmic);
    let mut group = c.benchmark_group("add-small-many-linear-to-polynomial");
    group.plot_config(plot_config.clone());
    for num_lin in [10, 100, 1000, 10_000] {
        let lins = (0..num_lin)
            .map(|_| -> Linear {
                random_deterministic(FunctionParameters {
                    num_terms: 3,
                    max_degree: 1,
                    max_id: 3 * num_lin as u64,
                })
            })
            .collect::<Vec<_>>();
        let poly: Polynomial = random_deterministic(FunctionParameters {
            num_terms: 3,
            max_degree: 3,
            max_id: (3 * num_lin) as u64,
        });
        group.bench_with_input(
            BenchmarkId::new("add-small-many-linear-to-polynomial", num_lin.to_string()),
            &(lins, poly),
            |b, (lins, poly)| {
                b.iter(|| lins.iter().fold(poly.clone(), |acc, lin| acc + lin.clone()))
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    sum_linear_small_many,
    sum_linear_large_little,
    sum_quadratic_small_many,
    sum_quadratic_large_little,
    sum_polynomial_small_many,
    sum_polynomial_large_little,
    add_linear_quadratic_large,
    add_small_many_linear_to_quadratic,
    add_small_many_linear_to_polynomial,
);
criterion_main!(benches);
