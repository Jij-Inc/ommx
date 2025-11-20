use crate::constraint::{Constraint, RemovedConstraint};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, PathExt};
use fnv::FnvHashMap;
use std::mem::size_of;

impl LogicalMemoryProfile for Constraint {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Count each field individually to avoid double-counting

        // id: ConstraintID (u64 wrapper)
        visitor.visit_leaf(path.with("id").as_ref(), size_of::<crate::ConstraintID>());

        // equality: Equality (enum)
        visitor.visit_leaf(path.with("equality").as_ref(), size_of::<crate::Equality>());

        // Delegate to Function
        self.function
            .visit_logical_memory(path.with("function").as_mut(), visitor);

        // name: Option<String>
        let name_bytes =
            size_of::<Option<String>>() + self.name.as_ref().map_or(0, |s| s.capacity());
        visitor.visit_leaf(path.with("name").as_ref(), name_bytes);

        // subscripts: Vec<i64>
        let subscripts_bytes =
            size_of::<Vec<i64>>() + self.subscripts.capacity() * size_of::<i64>();
        visitor.visit_leaf(path.with("subscripts").as_ref(), subscripts_bytes);

        // parameters: FnvHashMap<String, String>
        let map_overhead = size_of::<FnvHashMap<String, String>>();
        let mut entries_bytes = 0;
        for (k, v) in &self.parameters {
            entries_bytes += size_of::<(String, String)>();
            entries_bytes += k.capacity();
            entries_bytes += v.capacity();
        }
        let parameters_bytes = map_overhead + entries_bytes;
        visitor.visit_leaf(path.with("parameters").as_ref(), parameters_bytes);

        // description: Option<String>
        let description_bytes =
            size_of::<Option<String>>() + self.description.as_ref().map_or(0, |s| s.capacity());
        visitor.visit_leaf(path.with("description").as_ref(), description_bytes);
    }
}

impl LogicalMemoryProfile for RemovedConstraint {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Count each field individually to avoid double-counting

        // Delegate to Constraint
        path.push("constraint");
        self.constraint.visit_logical_memory(path, visitor);
        path.pop();

        // removed_reason: String
        path.push("removed_reason");
        let removed_reason_bytes = size_of::<String>() + self.removed_reason.capacity();
        visitor.visit_leaf(path, removed_reason_bytes);
        path.pop();

        // removed_reason_parameters: FnvHashMap<String, String>
        path.push("removed_reason_parameters");
        let map_overhead = size_of::<FnvHashMap<String, String>>();
        let mut entries_bytes = 0;
        for (k, v) in &self.removed_reason_parameters {
            entries_bytes += size_of::<(String, String)>();
            entries_bytes += k.capacity();
            entries_bytes += v.capacity();
        }
        let parameters_bytes = map_overhead + entries_bytes;
        visitor.visit_leaf(path, parameters_bytes);
        path.pop();
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
        insta::assert_snapshot!(folded, @r###"
        Constraint;description 24
        Constraint;equality 1
        Constraint;function;Linear;terms 104
        Constraint;id 8
        Constraint;name 24
        Constraint;parameters 32
        Constraint;subscripts 24
        "###);
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
        Constraint;description 41
        Constraint;equality 1
        Constraint;function;Linear;terms 104
        Constraint;id 8
        Constraint;name 39
        Constraint;parameters 32
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
        RemovedConstraint;constraint;description 24
        RemovedConstraint;constraint;equality 1
        RemovedConstraint;constraint;function;Linear;terms 104
        RemovedConstraint;constraint;id 8
        RemovedConstraint;constraint;name 24
        RemovedConstraint;constraint;parameters 32
        RemovedConstraint;constraint;subscripts 24
        RemovedConstraint;removed_reason 34
        RemovedConstraint;removed_reason_parameters 32
        "###);
    }
}
