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
        // Count each field individually to avoid double-counting

        // id: VariableID (u64 wrapper)
        path.push("id");
        visitor.visit_leaf(path, size_of::<crate::VariableID>());
        path.pop();

        // kind: Kind (enum)
        path.push("kind");
        visitor.visit_leaf(path, size_of::<crate::Kind>());
        path.pop();

        // bound: Bound (two f64s)
        path.push("bound");
        visitor.visit_leaf(path, size_of::<crate::Bound>());
        path.pop();

        // substituted_value: Option<f64>
        path.push("substituted_value");
        visitor.visit_leaf(path, size_of::<Option<f64>>());
        path.pop();

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
        // Count each field individually to avoid double-counting

        // name: Option<String> - count stack overhead
        path.push("name");
        let name_bytes = size_of::<Option<String>>()
            + self.name.as_ref().map_or(0, |s| s.capacity());
        visitor.visit_leaf(path, name_bytes);
        path.pop();

        // subscripts: Vec<i64> - count stack overhead + heap
        path.push("subscripts");
        let subscripts_bytes = size_of::<Vec<i64>>() + self.subscripts.capacity() * size_of::<i64>();
        visitor.visit_leaf(path, subscripts_bytes);
        path.pop();

        // parameters: FnvHashMap<String, String> - count stack overhead + heap
        path.push("parameters");
        let map_overhead = size_of::<FnvHashMap<String, String>>();
        let mut entries_bytes = 0;
        for (k, v) in &self.parameters {
            entries_bytes += size_of::<(String, String)>();
            entries_bytes += k.capacity();
            entries_bytes += v.capacity();
        }
        let parameters_bytes = map_overhead + entries_bytes;
        visitor.visit_leaf(path, parameters_bytes);
        path.pop();

        // description: Option<String> - count stack overhead
        path.push("description");
        let description_bytes = size_of::<Option<String>>()
            + self.description.as_ref().map_or(0, |s| s.capacity());
        visitor.visit_leaf(path, description_bytes);
        path.pop();
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
        insta::assert_snapshot!(folded, @r###"
        DecisionVariable;bound 16
        DecisionVariable;id 8
        DecisionVariable;kind 1
        DecisionVariable;metadata;description 24
        DecisionVariable;metadata;name 24
        DecisionVariable;metadata;parameters 32
        DecisionVariable;metadata;subscripts 24
        DecisionVariable;substituted_value 16
        "###);
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
        DecisionVariable;bound 16
        DecisionVariable;id 8
        DecisionVariable;kind 1
        DecisionVariable;metadata;description 38
        DecisionVariable;metadata;name 26
        DecisionVariable;metadata;parameters 32
        DecisionVariable;metadata;subscripts 48
        DecisionVariable;substituted_value 16
        "###);
    }
}
