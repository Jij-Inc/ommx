use crate::decision_variable::{DecisionVariable, DecisionVariableMetadata};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor};
use fnv::FnvHashMap;
use std::mem::size_of;

impl LogicalMemoryProfile for DecisionVariable {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Delegate to metadata
        path.push("metadata");
        self.metadata.visit_logical_memory(path, visitor);
        path.pop();
    }
}

impl LogicalMemoryProfile for DecisionVariableMetadata {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision_variable::{Kind, VariableID};
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{ATol, Bound};

    #[test]
    fn test_decision_variable_minimal_snapshot() {
        let dv = DecisionVariable::binary(VariableID::from(1));
        let folded = logical_memory_to_folded("DecisionVariable", &dv);
        // Empty metadata should produce no output
        insta::assert_snapshot!(folded, @"");
    }

    #[test]
    fn test_decision_variable_with_metadata_snapshot() {
        let mut dv = DecisionVariable::new(
            VariableID::from(1),
            Kind::Integer,
            Bound::new(0.0, 10.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();

        dv.metadata.name = Some("x1".to_string());
        dv.metadata.description = Some("First variable".to_string());
        dv.metadata.subscripts = vec![1, 2, 3];

        let folded = logical_memory_to_folded("DecisionVariable", &dv);
        insta::assert_snapshot!(folded, @r###"
        DecisionVariable;metadata;description 38
        DecisionVariable;metadata;name 26
        DecisionVariable;metadata;subscripts 48
        "###);
    }
}
