use crate::constraint_hints::{ConstraintHints, OneHot, Sos1};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use std::mem::size_of;

impl LogicalMemoryProfile for ConstraintHints {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // Count each field individually to avoid double-counting
        // Use "Type.field" format for flamegraph clarity

        // one_hot_constraints: Vec<OneHot>
        self.one_hot_constraints
            .visit_logical_memory(path.with("ConstraintHints.one_hot_constraints").as_mut(), visitor);

        // sos1_constraints: Vec<Sos1>
        self.sos1_constraints
            .visit_logical_memory(path.with("ConstraintHints.sos1_constraints").as_mut(), visitor);
    }
}

impl LogicalMemoryProfile for OneHot {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // Count each field individually to avoid double-counting
        // Use "Type.field" format for flamegraph clarity

        // id: ConstraintID (u64 wrapper)
        visitor.visit_leaf(&path.with("OneHot.id"), size_of::<crate::ConstraintID>());

        // variables: BTreeSet<VariableID>
        let set_overhead = size_of::<std::collections::BTreeSet<crate::VariableID>>();
        let elements_bytes = self.variables.len() * size_of::<crate::VariableID>();
        visitor.visit_leaf(&path.with("OneHot.variables"), set_overhead + elements_bytes);
    }
}

impl LogicalMemoryProfile for Sos1 {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // Count each field individually to avoid double-counting
        // Use "Type.field" format for flamegraph clarity

        // binary_constraint_id: ConstraintID (u64 wrapper)
        visitor.visit_leaf(
            &path.with("Sos1.binary_constraint_id"),
            size_of::<crate::ConstraintID>(),
        );

        // big_m_constraint_ids: BTreeSet<ConstraintID>
        let set_overhead = size_of::<std::collections::BTreeSet<crate::ConstraintID>>();
        let elements_bytes = self.big_m_constraint_ids.len() * size_of::<crate::ConstraintID>();
        visitor.visit_leaf(
            &path.with("Sos1.big_m_constraint_ids"),
            set_overhead + elements_bytes,
        );

        // variables: BTreeSet<VariableID>
        let set_overhead = size_of::<std::collections::BTreeSet<crate::VariableID>>();
        let elements_bytes = self.variables.len() * size_of::<crate::VariableID>();
        visitor.visit_leaf(&path.with("Sos1.variables"), set_overhead + elements_bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint_hints::{OneHot, Sos1};
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{ConstraintID, VariableID};

    #[test]
    fn test_constraint_hints_empty_snapshot() {
        let hints = ConstraintHints::default();
        let folded = logical_memory_to_folded(&hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints.one_hot_constraints;Vec[overhead] 24
        ConstraintHints.sos1_constraints;Vec[overhead] 24
        "###);
    }

    #[test]
    fn test_constraint_hints_with_one_hot_snapshot() {
        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables: [1, 2, 3].iter().map(|&id| VariableID::from(id)).collect(),
        };

        let hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        let folded = logical_memory_to_folded(&hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints.one_hot_constraints;OneHot.id 8
        ConstraintHints.one_hot_constraints;OneHot.variables 48
        ConstraintHints.one_hot_constraints;Vec[overhead] 24
        ConstraintHints.sos1_constraints;Vec[overhead] 24
        "###);
    }

    #[test]
    fn test_constraint_hints_with_sos1_snapshot() {
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(1),
            big_m_constraint_ids: [2, 3].iter().map(|&id| ConstraintID::from(id)).collect(),
            variables: [10, 11, 12]
                .iter()
                .map(|&id| VariableID::from(id))
                .collect(),
        };

        let hints = ConstraintHints {
            one_hot_constraints: vec![],
            sos1_constraints: vec![sos1],
        };

        let folded = logical_memory_to_folded(&hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints.one_hot_constraints;Vec[overhead] 24
        ConstraintHints.sos1_constraints;Sos1.big_m_constraint_ids 40
        ConstraintHints.sos1_constraints;Sos1.binary_constraint_id 8
        ConstraintHints.sos1_constraints;Sos1.variables 48
        ConstraintHints.sos1_constraints;Vec[overhead] 24
        "###);
    }
}
