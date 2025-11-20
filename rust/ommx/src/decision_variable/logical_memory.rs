use crate::decision_variable::{DecisionVariable, DecisionVariableMetadata, Kind, VariableID};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use std::mem::size_of;

impl LogicalMemoryProfile for VariableID {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<VariableID>());
    }
}

impl LogicalMemoryProfile for Kind {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<Kind>());
    }
}

crate::impl_logical_memory_profile! {
    DecisionVariable {
        id,
        kind,
        bound,
        substituted_value,
        metadata,
    }
}

crate::impl_logical_memory_profile! {
    DecisionVariableMetadata {
        name,
        subscripts,
        parameters,
        description,
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
        let folded = logical_memory_to_folded(&dv);
        // Empty metadata should produce no output
        insta::assert_snapshot!(folded, @r###"
        DecisionVariable.bound 16
        DecisionVariable.id 8
        DecisionVariable.kind 1
        DecisionVariable.metadata;DecisionVariableMetadata.description 24
        DecisionVariable.metadata;DecisionVariableMetadata.name 24
        DecisionVariable.metadata;DecisionVariableMetadata.parameters;FnvHashMap[overhead] 32
        DecisionVariable.metadata;DecisionVariableMetadata.subscripts;Vec[overhead] 24
        DecisionVariable.substituted_value 16
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

        let folded = logical_memory_to_folded(&dv);
        insta::assert_snapshot!(folded, @r###"
        DecisionVariable.bound 16
        DecisionVariable.id 8
        DecisionVariable.kind 1
        DecisionVariable.metadata;DecisionVariableMetadata.description 38
        DecisionVariable.metadata;DecisionVariableMetadata.name 26
        DecisionVariable.metadata;DecisionVariableMetadata.parameters;FnvHashMap[overhead] 32
        DecisionVariable.metadata;DecisionVariableMetadata.subscripts 24
        DecisionVariable.metadata;DecisionVariableMetadata.subscripts;Vec[overhead] 24
        DecisionVariable.substituted_value 16
        "###);
    }
}
