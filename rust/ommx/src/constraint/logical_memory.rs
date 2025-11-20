use crate::constraint::{Constraint, RemovedConstraint};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor};
use fnv::FnvHashMap;
use std::mem::size_of;

impl LogicalMemoryProfile for Constraint {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Delegate to Function
        path.push("function");
        self.function.visit_logical_memory(path, visitor);
        path.pop();

        // String fields: name and description
        if let Some(name) = &self.name {
            path.push("name");
            let bytes = size_of::<String>() + name.capacity();
            visitor.visit_leaf(path, bytes);
            path.pop();
        }

        if let Some(description) = &self.description {
            path.push("description");
            let bytes = size_of::<String>() + description.capacity();
            visitor.visit_leaf(path, bytes);
            path.pop();
        }

        // Vec<i64> subscripts
        if !self.subscripts.is_empty() {
            path.push("subscripts");
            let bytes = size_of::<Vec<i64>>() + self.subscripts.capacity() * size_of::<i64>();
            visitor.visit_leaf(path, bytes);
            path.pop();
        }

        // FnvHashMap<String, String> parameters
        if !self.parameters.is_empty() {
            path.push("parameters");
            let map_overhead = size_of::<FnvHashMap<String, String>>();
            let mut entries_bytes = 0;
            for (k, v) in &self.parameters {
                entries_bytes += size_of::<(String, String)>();
                entries_bytes += k.capacity();
                entries_bytes += v.capacity();
            }
            let total_bytes = map_overhead + entries_bytes;
            visitor.visit_leaf(path, total_bytes);
            path.pop();
        }
    }
}

impl LogicalMemoryProfile for RemovedConstraint {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Delegate to Constraint
        path.push("constraint");
        self.constraint.visit_logical_memory(path, visitor);
        path.pop();

        // String field: removed_reason
        path.push("removed_reason");
        let bytes = size_of::<String>() + self.removed_reason.capacity();
        visitor.visit_leaf(path, bytes);
        path.pop();

        // FnvHashMap<String, String> removed_reason_parameters
        if !self.removed_reason_parameters.is_empty() {
            path.push("removed_reason_parameters");
            let map_overhead = size_of::<FnvHashMap<String, String>>();
            let mut entries_bytes = 0;
            for (k, v) in &self.removed_reason_parameters {
                entries_bytes += size_of::<(String, String)>();
                entries_bytes += k.capacity();
                entries_bytes += v.capacity();
            }
            let total_bytes = map_overhead + entries_bytes;
            visitor.visit_leaf(path, total_bytes);
            path.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::ConstraintID;
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{coeff, linear, Function};

    #[test]
    fn test_constraint_snapshot() {
        let constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Linear(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2)),
        );
        let folded = logical_memory_to_folded("Constraint", &constraint);
        insta::assert_snapshot!(folded, @"Constraint;function;Linear;terms 104");
    }

    #[test]
    fn test_constraint_with_metadata_snapshot() {
        let mut constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Linear(coeff!(2.0) * linear!(1)),
        );
        constraint.name = Some("test_constraint".to_string());
        constraint.description = Some("A test constraint".to_string());
        constraint.subscripts = vec![1, 2, 3];

        let folded = logical_memory_to_folded("Constraint", &constraint);
        // Should include function, name, description, and subscripts
        insta::assert_snapshot!(folded, @r###"
        Constraint;function;Linear;terms 104
        Constraint;name 39
        Constraint;description 41
        Constraint;subscripts 48
        "###);
    }

    #[test]
    fn test_removed_constraint_snapshot() {
        let constraint = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(2),
            Function::Linear(coeff!(1.0) * linear!(3)),
        );
        let removed = RemovedConstraint {
            constraint,
            removed_reason: "infeasible".to_string(),
            removed_reason_parameters: Default::default(),
        };

        let folded = logical_memory_to_folded("RemovedConstraint", &removed);
        insta::assert_snapshot!(folded, @r###"
        RemovedConstraint;constraint;function;Linear;terms 104
        RemovedConstraint;removed_reason 34
        "###);
    }
}
