use super::{
    logical_memory_to_folded, logical_total_bytes, LogicalMemoryProfile, LogicalMemoryVisitor,
    MemoryProfile, Path,
};
use crate::{coeff, linear, quadratic, Linear, Quadratic};

// Unit tests for core collectors

#[test]
fn test_memory_profile_aggregation() {
    let mut profile = MemoryProfile::default();
    profile.visit_leaf(&Path::from(vec!["root", "child", "leaf"]), 1024);
    profile.visit_leaf(&Path::from(vec!["root", "child", "other"]), 2048);

    let output = profile.to_string();
    assert_eq!(output, "root;child;leaf 1024\nroot;child;other 2048");
    assert_eq!(profile.total_bytes(), 3072);
    assert_eq!(profile.len(), 2);
}

#[test]
fn test_memory_profile_skip_zero() {
    let mut profile = MemoryProfile::default();
    profile.visit_leaf(&Path::from(vec!["root", "empty"]), 0);
    profile.visit_leaf(&Path::from(vec!["root", "nonempty"]), 100);

    let output = profile.to_string();
    assert_eq!(output, "root;nonempty 100");
    assert_eq!(profile.total_bytes(), 100);
}

#[test]
fn test_memory_profile_same_path_aggregates() {
    let mut profile = MemoryProfile::default();
    profile.visit_leaf(&Path::from(vec!["root", "leaf"]), 10);
    profile.visit_leaf(&Path::from(vec!["root", "leaf"]), 32);

    // Multiple visits to the same path accumulate
    assert_eq!(profile.to_string(), "root;leaf 42");
    assert_eq!(profile.total_bytes(), 42);
    assert_eq!(profile.len(), 1);
}

#[test]
fn test_memory_profile_entries_iter() {
    let mut profile = MemoryProfile::default();
    profile.visit_leaf(&Path::from(vec!["a"]), 1);
    profile.visit_leaf(&Path::from(vec!["b"]), 2);

    let collected: Vec<(Vec<&'static str>, usize)> =
        profile.entries().map(|(p, b)| (p.to_vec(), b)).collect();
    assert_eq!(collected, vec![(vec!["a"], 1), (vec!["b"], 2),]);
}

// Integration tests for polynomial types

#[test]
fn test_linear_memory_profile() {
    // Create a linear expression: 2*x1 + 3*x2 + 5
    let expr: Linear = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(5.0);

    let folded = logical_memory_to_folded(&expr);
    insta::assert_snapshot!(folded, @"PolynomialBase.terms 104");

    let total = logical_total_bytes(&expr);
    assert!(total > 0);
}

#[test]
fn test_linear_snapshot() {
    // Create a deterministic linear expression: 2*x1 + 3*x2 + 5
    let expr: Linear = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(5.0);

    let folded = logical_memory_to_folded(&expr);

    // Snapshot test: verify the exact format of folded stack output
    insta::assert_snapshot!(folded, @"PolynomialBase.terms 104");
}

#[test]
fn test_quadratic_memory_profile() {
    // Create a quadratic expression: x1*x2 + 2*x1 + 1
    let expr: Quadratic =
        coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1) + coeff!(1.0);

    let folded = logical_memory_to_folded(&expr);
    insta::assert_snapshot!(folded, @"PolynomialBase.terms 128");

    let total = logical_total_bytes(&expr);
    assert!(total > 0);
}

#[test]
fn test_quadratic_snapshot() {
    // Create a deterministic quadratic expression: x1*x2 + 2*x1 + 1
    let expr: Quadratic =
        coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1) + coeff!(1.0);

    let folded = logical_memory_to_folded(&expr);

    // Snapshot test: verify the exact format of folded stack output
    insta::assert_snapshot!(folded, @"PolynomialBase.terms 128");
}

#[test]
fn test_large_linear_memory() {
    // Create a larger linear expression with many terms
    let mut expr = coeff!(1.0) * linear!(1);
    for i in 2..=100 {
        expr += coeff!(i as f64) * linear!(i);
    }

    let folded = logical_memory_to_folded(&expr);
    println!("Large Linear folded stack:\n{}", folded);

    let total = logical_total_bytes(&expr);
    println!("Large Linear total bytes: {}", total);

    // With 100 terms, memory should be substantial
    assert!(total > 1000); // At least 1KB
}

#[test]
fn test_medium_linear_snapshot() {
    // Create a medium-sized deterministic linear expression with 10 terms
    let mut expr = coeff!(1.0) * linear!(1);
    for i in 2..=10 {
        expr += coeff!(i as f64) * linear!(i);
    }

    let folded = logical_memory_to_folded(&expr);

    // Snapshot test for medium-sized expression
    insta::assert_snapshot!(folded, @"PolynomialBase.terms 272");
}

// Tests for generic collection implementations

#[test]
fn test_btreemap_with_linear() {
    use crate::VariableID;
    use std::collections::BTreeMap;

    let mut map = BTreeMap::new();
    map.insert(VariableID::from(1), coeff!(2.0) * linear!(1));
    map.insert(VariableID::from(2), coeff!(3.0) * linear!(2));

    let folded = logical_memory_to_folded(&map);
    insta::assert_snapshot!(folded, @r###"
    BTreeMap[key] 16
    BTreeMap[stack] 24
    PolynomialBase.terms 112
    "###);
}

#[test]
fn test_hashmap_with_linear() {
    use crate::VariableID;
    use std::collections::HashMap;

    let mut map = HashMap::new();
    map.insert(VariableID::from(1), coeff!(2.0) * linear!(1));
    map.insert(VariableID::from(2), coeff!(3.0) * linear!(2));

    let folded = logical_memory_to_folded(&map);
    // Note: HashMap iteration order is non-deterministic, but snapshots should still be stable
    insta::assert_snapshot!(folded, @r###"
    HashMap[key] 16
    HashMap[stack] 48
    PolynomialBase.terms 112
    "###);
}

#[test]
fn test_vec_with_linear() {
    let vec = vec![
        coeff!(2.0) * linear!(1),
        coeff!(3.0) * linear!(2),
        coeff!(4.0) * linear!(3),
    ];

    let folded = logical_memory_to_folded(&vec);
    insta::assert_snapshot!(folded, @r###"
    PolynomialBase.terms 168
    Vec[stack] 24
    "###);
}

#[test]
fn test_empty_collections() {
    use crate::Linear;
    use std::collections::BTreeMap;

    let empty_map: BTreeMap<u64, Linear> = BTreeMap::new();
    let folded = logical_memory_to_folded(&empty_map);
    // Empty map should only have struct overhead
    insta::assert_snapshot!(folded, @"BTreeMap[stack] 24");

    let empty_vec: Vec<Linear> = Vec::new();
    let folded_vec = logical_memory_to_folded(&empty_vec);
    // Empty vec should only have struct overhead
    insta::assert_snapshot!(folded_vec, @"Vec[stack] 24");
}

// Tests for the #[derive(LogicalMemoryProfile)] macro.
//
// These confirm that the proc-macro-generated impl matches the behavior of
// the declarative `impl_logical_memory_profile!` macro: fields are emitted
// under `Type.field` frames, and delegation to nested types preserves the
// path hierarchy.

#[derive(LogicalMemoryProfile)]
struct DeriveTargetFlat {
    alpha: u64,
    beta: f64,
    gamma: String,
}

#[derive(LogicalMemoryProfile)]
struct DeriveTargetNested {
    leaf: u32,
    inner: DeriveTargetFlat,
}

#[test]
fn test_derive_flat_struct_snapshot() {
    // u64=8, f64=8, String: size_of<String>=24 + len("hi")=2 → 26
    let value = DeriveTargetFlat {
        alpha: 0,
        beta: 0.0,
        gamma: "hi".to_string(),
    };
    let folded = logical_memory_to_folded(&value);
    insta::assert_snapshot!(folded, @r###"
    DeriveTargetFlat.alpha 8
    DeriveTargetFlat.beta 8
    DeriveTargetFlat.gamma 26
    "###);
}

#[test]
fn test_derive_nested_struct_snapshot() {
    // Nested struct: the outer type emits `DeriveTargetNested.inner` as the
    // frame, then the inner `#[derive]` emits its own `DeriveTargetFlat.*`
    // segments under that.
    let value = DeriveTargetNested {
        leaf: 7,
        inner: DeriveTargetFlat {
            alpha: 0,
            beta: 0.0,
            gamma: String::new(),
        },
    };
    let folded = logical_memory_to_folded(&value);
    insta::assert_snapshot!(folded, @r###"
    DeriveTargetNested.inner;DeriveTargetFlat.alpha 8
    DeriveTargetNested.inner;DeriveTargetFlat.beta 8
    DeriveTargetNested.inner;DeriveTargetFlat.gamma 24
    DeriveTargetNested.leaf 4
    "###);
}

#[test]
fn test_derive_matches_declarative_macro() {
    // The derive and the declarative `impl_logical_memory_profile!` macro
    // must produce identical output for the same field set. This guards
    // against regressions if one of the two implementations drifts.
    struct Target {
        a: u64,
        b: u32,
    }
    crate::impl_logical_memory_profile! {
        Target { a, b }
    }

    #[derive(LogicalMemoryProfile)]
    struct TargetDerived {
        a: u64,
        b: u32,
    }

    let declarative = logical_memory_to_folded(&Target { a: 1, b: 2 });
    let derived = logical_memory_to_folded(&TargetDerived { a: 1, b: 2 });

    // Swap the type name so the two outputs are directly comparable.
    let derived_renamed = derived.replace("TargetDerived", "Target");
    assert_eq!(declarative, derived_renamed);
}
