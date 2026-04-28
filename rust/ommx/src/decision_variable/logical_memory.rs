use crate::decision_variable::{Kind, VariableID};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use std::mem::size_of;

impl LogicalMemoryProfile for VariableID {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<VariableID>());
    }
}

impl LogicalMemoryProfile for Kind {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<Kind>());
    }
}

// DecisionVariable and DecisionVariableMetadata use
// `#[derive(LogicalMemoryProfile)]` on their definition sites.

#[cfg(test)]
mod tests {
    use crate::decision_variable::{DecisionVariable, Kind, VariableID};
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{ATol, Bound};

    #[test]
    fn test_decision_variable_minimal_snapshot() {
        let dv = DecisionVariable::binary(VariableID::from(1));
        let folded = logical_memory_to_folded(&dv);
        // Per-element metadata storage was retired in v3 — only the
        // intrinsic fields appear here; per-variable metadata lives at
        // `Instance::variable_metadata` (see `instance/logical_memory.rs`).
        insta::assert_snapshot!(folded, @r###"
        DecisionVariable.bound 16
        DecisionVariable.id 8
        DecisionVariable.kind 1
        DecisionVariable.substituted_value;Option[stack] 16
        "###);
    }

    // The previous `test_decision_variable_with_metadata_snapshot` exercised
    // per-element `DecisionVariable.metadata` storage, which was retired
    // in v3. Per-variable metadata is now accounted for at the
    // `Instance::variable_metadata` SoA-store level.
    #[test]
    fn test_decision_variable_minimal_no_metadata_snapshot() {
        let dv = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        let folded = logical_memory_to_folded(&dv);
        insta::assert_snapshot!(folded, @r###"
        DecisionVariable.bound 16
        DecisionVariable.id 8
        DecisionVariable.kind 1
        DecisionVariable.substituted_value;Option[stack] 16
        "###);
    }
}
