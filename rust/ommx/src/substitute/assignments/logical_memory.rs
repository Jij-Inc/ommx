use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use crate::substitute::AcyclicAssignments;
use std::mem::size_of;

impl LogicalMemoryProfile for AcyclicAssignments {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // Count each field individually to avoid double-counting
        // Use "Type.field" format for flamegraph clarity

        // assignments: FnvHashMap<VariableID, Function>
        self.assignments.visit_logical_memory(
            path.with("AcyclicAssignments.assignments").as_mut(),
            visitor,
        );

        // dependency: DiGraphMap<VariableID, ()>
        // Estimate: node count * size_of::<VariableID>() + edge count * (size_of::<VariableID>() * 2)
        let graph_overhead = size_of::<petgraph::graphmap::DiGraphMap<crate::VariableID, ()>>();
        let node_bytes = self.dependency.node_count() * size_of::<crate::VariableID>();
        let edge_bytes = self.dependency.edge_count() * (size_of::<crate::VariableID>() * 2);
        let total_bytes = graph_overhead + node_bytes + edge_bytes;
        visitor.visit_leaf(&path.with("AcyclicAssignments.dependency"), total_bytes);
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
        let folded = logical_memory_to_folded(&assignments);
        // Empty assignments should produce no output
        insta::assert_snapshot!(folded, @r###"
        AcyclicAssignments.assignments;FnvHashMap[stack] 32
        AcyclicAssignments.dependency 144
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

        let folded = logical_memory_to_folded(&assignments);
        insta::assert_snapshot!(folded, @r###"
        AcyclicAssignments.assignments;FnvHashMap[key] 16
        AcyclicAssignments.assignments;FnvHashMap[stack] 32
        AcyclicAssignments.assignments;Linear;PolynomialBase.terms 160
        AcyclicAssignments.dependency 224
        "###);
    }
}
