use crate::decision_variable::{DecisionVariable, DecisionVariableMetadata};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use fnv::FnvHashMap;
use std::mem::size_of;

impl LogicalMemoryProfile for DecisionVariable {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Path,
        visitor: &mut V,
    ) {
        // Count each field individually to avoid double-counting

        // id: VariableID (u64 wrapper)
        visitor.visit_leaf(&path.with("id"), size_of::<crate::VariableID>());

        // kind: Kind (enum)
        visitor.visit_leaf(&path.with("kind"), size_of::<crate::Kind>());

        // bound: Bound (two f64s)
        visitor.visit_leaf(&path.with("bound"), size_of::<crate::Bound>());

        // substituted_value: Option<f64>
        visitor.visit_leaf(&path.with("substituted_value"), size_of::<Option<f64>>());

        // Delegate to metadata
        self.metadata
            .visit_logical_memory(path.with("metadata").as_mut(), visitor);
    }
}

impl LogicalMemoryProfile for DecisionVariableMetadata {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Path,
        visitor: &mut V,
    ) {
        // Count each field individually to avoid double-counting

        // name: Option<String> - count stack overhead
        let name_bytes = size_of::<Option<String>>()
            + self.name.as_ref().map_or(0, |s| s.capacity());
        visitor.visit_leaf(&path.with("name"), name_bytes);

        // subscripts: Vec<i64> - count stack overhead + heap
        let subscripts_bytes = size_of::<Vec<i64>>() + self.subscripts.capacity() * size_of::<i64>();
        visitor.visit_leaf(&path.with("subscripts"), subscripts_bytes);

        // parameters: FnvHashMap<String, String> - count stack overhead + heap
        let map_overhead = size_of::<FnvHashMap<String, String>>();
        let mut entries_bytes = 0;
        for (k, v) in &self.parameters {
            entries_bytes += size_of::<(String, String)>();
            entries_bytes += k.capacity();
            entries_bytes += v.capacity();
        }
        let parameters_bytes = map_overhead + entries_bytes;
        visitor.visit_leaf(&path.with("parameters"), parameters_bytes);

        // description: Option<String> - count stack overhead
        let description_bytes = size_of::<Option<String>>()
            + self.description.as_ref().map_or(0, |s| s.capacity());
        visitor.visit_leaf(&path.with("description"), description_bytes);
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
