use crate::constraint_hints::{ConstraintHints, OneHot, Sos1};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor};
use std::mem::size_of;

impl LogicalMemoryProfile for ConstraintHints {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Vec<OneHot> one_hot_constraints
        if !self.one_hot_constraints.is_empty() {
            path.push("one_hot_constraints");
            for one_hot in &self.one_hot_constraints {
                one_hot.visit_logical_memory(path, visitor);
            }
            path.pop();
        }

        // Vec<Sos1> sos1_constraints
        if !self.sos1_constraints.is_empty() {
            path.push("sos1_constraints");
            for sos1 in &self.sos1_constraints {
                sos1.visit_logical_memory(path, visitor);
            }
            path.pop();
        }
    }
}

impl LogicalMemoryProfile for OneHot {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // BTreeSet<VariableID> variables
        if !self.variables.is_empty() {
            path.push("variables");
            // BTreeSet overhead + number of elements * size of element
            let bytes = size_of::<std::collections::BTreeSet<crate::VariableID>>()
                + self.variables.len() * size_of::<crate::VariableID>();
            visitor.visit_leaf(path, bytes);
            path.pop();
        }
    }
}

impl LogicalMemoryProfile for Sos1 {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // BTreeSet<ConstraintID> big_m_constraint_ids
        if !self.big_m_constraint_ids.is_empty() {
            path.push("big_m_constraint_ids");
            let bytes = size_of::<std::collections::BTreeSet<crate::ConstraintID>>()
                + self.big_m_constraint_ids.len() * size_of::<crate::ConstraintID>();
            visitor.visit_leaf(path, bytes);
            path.pop();
        }

        // BTreeSet<VariableID> variables
        if !self.variables.is_empty() {
            path.push("variables");
            let bytes = size_of::<std::collections::BTreeSet<crate::VariableID>>()
                + self.variables.len() * size_of::<crate::VariableID>();
            visitor.visit_leaf(path, bytes);
            path.pop();
        }
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
        insta::assert_snapshot!(folded, @"");
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
        insta::assert_snapshot!(folded, @"ConstraintHints;one_hot_constraints;variables 48");
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
        ConstraintHints;sos1_constraints;big_m_constraint_ids 40
        ConstraintHints;sos1_constraints;variables 48
        "###);
    }
}
