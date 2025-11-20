use crate::constraint_hints::{ConstraintHints, OneHot, Sos1};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor};
use std::mem::size_of;

impl LogicalMemoryProfile for ConstraintHints {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Count each field individually to avoid double-counting

        // one_hot_constraints: Vec<OneHot>
        path.push("one_hot_constraints");
        let vec_overhead = size_of::<Vec<OneHot>>();
        visitor.visit_leaf(path, vec_overhead);
        for one_hot in &self.one_hot_constraints {
            path.push("OneHot");
            one_hot.visit_logical_memory(path, visitor);
            path.pop();
        }
        path.pop();

        // sos1_constraints: Vec<Sos1>
        path.push("sos1_constraints");
        let vec_overhead = size_of::<Vec<Sos1>>();
        visitor.visit_leaf(path, vec_overhead);
        for sos1 in &self.sos1_constraints {
            path.push("Sos1");
            sos1.visit_logical_memory(path, visitor);
            path.pop();
        }
        path.pop();
    }
}

impl LogicalMemoryProfile for OneHot {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Count each field individually to avoid double-counting

        // id: ConstraintID (u64 wrapper)
        path.push("id");
        visitor.visit_leaf(path, size_of::<crate::ConstraintID>());
        path.pop();

        // variables: BTreeSet<VariableID>
        path.push("variables");
        let set_overhead = size_of::<std::collections::BTreeSet<crate::VariableID>>();
        let elements_bytes = self.variables.len() * size_of::<crate::VariableID>();
        visitor.visit_leaf(path, set_overhead + elements_bytes);
        path.pop();
    }
}

impl LogicalMemoryProfile for Sos1 {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Count each field individually to avoid double-counting

        // binary_constraint_id: ConstraintID (u64 wrapper)
        path.push("binary_constraint_id");
        visitor.visit_leaf(path, size_of::<crate::ConstraintID>());
        path.pop();

        // big_m_constraint_ids: BTreeSet<ConstraintID>
        path.push("big_m_constraint_ids");
        let set_overhead = size_of::<std::collections::BTreeSet<crate::ConstraintID>>();
        let elements_bytes = self.big_m_constraint_ids.len() * size_of::<crate::ConstraintID>();
        visitor.visit_leaf(path, set_overhead + elements_bytes);
        path.pop();

        // variables: BTreeSet<VariableID>
        path.push("variables");
        let set_overhead = size_of::<std::collections::BTreeSet<crate::VariableID>>();
        let elements_bytes = self.variables.len() * size_of::<crate::VariableID>();
        visitor.visit_leaf(path, set_overhead + elements_bytes);
        path.pop();
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
        let folded = logical_memory_to_folded("ConstraintHints", &hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints;one_hot_constraints 24
        ConstraintHints;sos1_constraints 24
        "###);
    }

    #[test]
    fn test_constraint_hints_with_one_hot_snapshot() {
        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables: [1, 2, 3]
                .iter()
                .map(|&id| VariableID::from(id))
                .collect(),
        };

        let hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        let folded = logical_memory_to_folded("ConstraintHints", &hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints;one_hot_constraints 24
        ConstraintHints;one_hot_constraints;OneHot;id 8
        ConstraintHints;one_hot_constraints;OneHot;variables 48
        ConstraintHints;sos1_constraints 24
        "###);
    }

    #[test]
    fn test_constraint_hints_with_sos1_snapshot() {
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(1),
            big_m_constraint_ids: [2, 3]
                .iter()
                .map(|&id| ConstraintID::from(id))
                .collect(),
            variables: [10, 11, 12]
                .iter()
                .map(|&id| VariableID::from(id))
                .collect(),
        };

        let hints = ConstraintHints {
            one_hot_constraints: vec![],
            sos1_constraints: vec![sos1],
        };

        let folded = logical_memory_to_folded("ConstraintHints", &hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints;one_hot_constraints 24
        ConstraintHints;sos1_constraints 24
        ConstraintHints;sos1_constraints;Sos1;big_m_constraint_ids 40
        ConstraintHints;sos1_constraints;Sos1;binary_constraint_id 8
        ConstraintHints;sos1_constraints;Sos1;variables 48
        "###);
    }
}
