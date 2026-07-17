// DecisionVariable and DecisionVariableLabel implement logical-memory
// profiling on their definition sites.

#[cfg(test)]
mod tests {
    use crate::decision_variable::{DecisionVariable, Kind};
    use crate::logical_memory::logical_memory_to_folded;
    use crate::Bound;

    #[test]
    fn test_decision_variable_minimal_snapshot() {
        let dv = DecisionVariable::binary();
        let folded = logical_memory_to_folded(&dv);
        // Per-element label storage was retired in v3 — only the
        // intrinsic fields appear here; per-variable modeling labels live at
        // `Instance::variable_labels` (see `instance/logical_memory.rs`).
        insta::assert_snapshot!(folded, @r###"
        DecisionVariable.bound;Bound.lower 8
        DecisionVariable.bound;Bound.upper 8
        DecisionVariable.kind 1
        "###);
    }

    // The previous `test_decision_variable_with_label_snapshot` exercised
    // per-element `DecisionVariable` label storage, which was retired
    // in v3. Per-variable modeling labels are now accounted for at the
    // `Instance::variable_labels` SoA-store level.
    #[test]
    fn test_decision_variable_minimal_no_label_snapshot() {
        let dv = DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, 10.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();

        let folded = logical_memory_to_folded(&dv);
        insta::assert_snapshot!(folded, @r###"
        DecisionVariable.bound;Bound.lower 8
        DecisionVariable.bound;Bound.upper 8
        DecisionVariable.kind 1
        "###);
    }

    #[test]
    fn test_finite_domain_snapshot() {
        let dv = DecisionVariable::new_finite_domain(vec![0.1, 0.5, 1.0]).unwrap();
        let folded = logical_memory_to_folded(&dv);

        insta::assert_snapshot!(folded, @r###"
        DecisionVariable.finite_domain;FiniteDomain.bound;Bound.lower 8
        DecisionVariable.finite_domain;FiniteDomain.bound;Bound.upper 8
        DecisionVariable.finite_domain;FiniteDomain.values 24
        DecisionVariable.finite_domain;FiniteDomain.values;Vec[stack] 24
        DecisionVariable.kind 1
        "###);
    }
}
