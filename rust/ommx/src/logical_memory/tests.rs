use super::{
    logical_memory_to_folded, logical_total_bytes, FoldedCollector, LogicalMemoryVisitor, Path,
};
use crate::{coeff, linear, quadratic, Linear, Quadratic};

// Unit tests for core collectors

#[test]
fn test_folded_collector() {
    let mut collector = FoldedCollector::new();
    collector.visit_leaf(&Path::from(vec!["root", "child", "leaf"]), 1024);
    collector.visit_leaf(&Path::from(vec!["root", "child", "other"]), 2048);

    let output = collector.finish();
    assert_eq!(output, "root;child;leaf 1024\nroot;child;other 2048");
}

#[test]
fn test_folded_collector_skip_zero() {
    let mut collector = FoldedCollector::new();
    collector.visit_leaf(&Path::from(vec!["root", "empty"]), 0);
    collector.visit_leaf(&Path::from(vec!["root", "nonempty"]), 100);

    let output = collector.finish();
    assert_eq!(output, "root;nonempty 100");
}

// Integration tests for polynomial types

#[test]
fn test_linear_memory_profile() {
    // Create a linear expression: 2*x1 + 3*x2 + 5
    let expr: Linear = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(5.0);

    let folded = logical_memory_to_folded("Linear", &expr);
    println!("Linear folded stack:\n{}", folded);

    let total = logical_total_bytes("Linear", &expr);
    println!("Linear total bytes: {}", total);

    assert!(folded.contains("Linear;terms"));
    assert!(total > 0);
}

#[test]
fn test_linear_snapshot() {
    // Create a deterministic linear expression: 2*x1 + 3*x2 + 5
    let expr: Linear = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(5.0);

    let folded = logical_memory_to_folded("Linear", &expr);

    // Snapshot test: verify the exact format of folded stack output
    insta::assert_snapshot!(folded, @"Linear;terms 104");
}

#[test]
fn test_quadratic_memory_profile() {
    // Create a quadratic expression: x1*x2 + 2*x1 + 1
    let expr: Quadratic =
        coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1) + coeff!(1.0);

    let folded = logical_memory_to_folded("Quadratic", &expr);
    println!("Quadratic folded stack:\n{}", folded);

    let total = logical_total_bytes("Quadratic", &expr);
    println!("Quadratic total bytes: {}", total);

    assert!(folded.contains("Quadratic;terms"));
    assert!(total > 0);
}

#[test]
fn test_quadratic_snapshot() {
    // Create a deterministic quadratic expression: x1*x2 + 2*x1 + 1
    let expr: Quadratic =
        coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1) + coeff!(1.0);

    let folded = logical_memory_to_folded("Quadratic", &expr);

    // Snapshot test: verify the exact format of folded stack output
    insta::assert_snapshot!(folded, @"Quadratic;terms 128");
}

#[test]
fn test_large_linear_memory() {
    // Create a larger linear expression with many terms
    let mut expr = coeff!(1.0) * linear!(1);
    for i in 2..=100 {
        expr = expr + coeff!(i as f64) * linear!(i);
    }

    let folded = logical_memory_to_folded("Linear", &expr);
    println!("Large Linear folded stack:\n{}", folded);

    let total = logical_total_bytes("Linear", &expr);
    println!("Large Linear total bytes: {}", total);

    // With 100 terms, memory should be substantial
    assert!(total > 1000); // At least 1KB
}

#[test]
fn test_medium_linear_snapshot() {
    // Create a medium-sized deterministic linear expression with 10 terms
    let mut expr = coeff!(1.0) * linear!(1);
    for i in 2..=10 {
        expr = expr + coeff!(i as f64) * linear!(i);
    }

    let folded = logical_memory_to_folded("Linear", &expr);

    // Snapshot test for medium-sized expression
    insta::assert_snapshot!(folded, @"Linear;terms 272");
}
