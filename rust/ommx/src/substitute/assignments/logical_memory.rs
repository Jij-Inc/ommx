use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor};
use crate::substitute::AcyclicAssignments;
use std::mem::size_of;

impl LogicalMemoryProfile for AcyclicAssignments {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Count each field individually to avoid double-counting

        // assignments: FnvHashMap<VariableID, Function>
        path.push("assignments");

        // HashMap stack overhead
        let map_overhead = size_of::<fnv::FnvHashMap<crate::VariableID, crate::Function>>();
        visitor.visit_leaf(path, map_overhead);

        // Keys (VariableID)
        path.push("keys");
        let key_size = size_of::<crate::VariableID>();
        let keys_bytes = self.assignments.len() * key_size;
        visitor.visit_leaf(path, keys_bytes);
        path.pop();

        // Delegate to each Function
        for function in self.assignments.values() {
            path.push("Function");
            function.visit_logical_memory(path, visitor);
            path.pop();
        }

        path.pop();

        // dependency: DiGraphMap<VariableID, ()>
        // Estimate: node count * size_of::<VariableID>() + edge count * (size_of::<VariableID>() * 2)
        path.push("dependency");

        let graph_overhead = size_of::<petgraph::graphmap::DiGraphMap<crate::VariableID, ()>>();
        let node_bytes = self.dependency.node_count() * size_of::<crate::VariableID>();
        let edge_bytes = self.dependency.edge_count() * (size_of::<crate::VariableID>() * 2);
        let total_bytes = graph_overhead + node_bytes + edge_bytes;
        visitor.visit_leaf(path, total_bytes);

        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{assign, coeff, linear};

    #[test]
    fn test_acyclic_assignments_empty_snapshot() {
        let assignments = AcyclicAssignments::default();
        let folded = logical_memory_to_folded("Assignments", &assignments);
        // Empty assignments should produce no output
        insta::assert_snapshot!(folded, @r###"
        Assignments;assignments 32
        Assignments;dependency 144
        "###);
    }

    #[test]
    fn test_acyclic_assignments_snapshot() {
        // x1 <- x2 + x3
        // x4 <- x1 + 2
        let assignments = assign! {
            1 <- linear!(2) + linear!(3),
            4 <- linear!(1) + coeff!(2.0)
        };

        let folded = logical_memory_to_folded("Assignments", &assignments);
        insta::assert_snapshot!(folded, @r###"
        Assignments;assignments 32
        Assignments;assignments;Function;Linear;terms 208
        Assignments;assignments;keys 16
        Assignments;dependency 224
        "###);
    }
}
