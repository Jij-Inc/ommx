use crate::constraint_hints::{ConstraintHints, OneHot, Sos1};

crate::impl_logical_memory_profile! {
    ConstraintHints {
        one_hot_constraints,
        sos1_constraints,
    }
}

crate::impl_logical_memory_profile! {
    OneHot {
        id,
        variables,
    }
}

crate::impl_logical_memory_profile! {
    Sos1 {
        binary_constraint_id,
        big_m_constraint_ids,
        variables,
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
        let folded = logical_memory_to_folded(&hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints.one_hot_constraints;Vec[overhead] 24
        ConstraintHints.sos1_constraints;Vec[overhead] 24
        "###);
    }

    #[test]
    fn test_constraint_hints_with_one_hot_snapshot() {
        let one_hot = OneHot {
            id: ConstraintID::from(1),
            variables: [1, 2, 3].iter().map(|&id| VariableID::from(id)).collect(),
        };

        let hints = ConstraintHints {
            one_hot_constraints: vec![one_hot],
            sos1_constraints: vec![],
        };

        let folded = logical_memory_to_folded(&hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints.one_hot_constraints;OneHot.id 8
        ConstraintHints.one_hot_constraints;OneHot.variables 24
        ConstraintHints.one_hot_constraints;OneHot.variables;BTreeSet[overhead] 24
        ConstraintHints.one_hot_constraints;Vec[overhead] 24
        ConstraintHints.sos1_constraints;Vec[overhead] 24
        "###);
    }

    #[test]
    fn test_constraint_hints_with_sos1_snapshot() {
        let sos1 = Sos1 {
            binary_constraint_id: ConstraintID::from(1),
            big_m_constraint_ids: [2, 3].iter().map(|&id| ConstraintID::from(id)).collect(),
            variables: [10, 11, 12]
                .iter()
                .map(|&id| VariableID::from(id))
                .collect(),
        };

        let hints = ConstraintHints {
            one_hot_constraints: vec![],
            sos1_constraints: vec![sos1],
        };

        let folded = logical_memory_to_folded(&hints);
        insta::assert_snapshot!(folded, @r###"
        ConstraintHints.one_hot_constraints;Vec[overhead] 24
        ConstraintHints.sos1_constraints;Sos1.big_m_constraint_ids 16
        ConstraintHints.sos1_constraints;Sos1.big_m_constraint_ids;BTreeSet[overhead] 24
        ConstraintHints.sos1_constraints;Sos1.binary_constraint_id 8
        ConstraintHints.sos1_constraints;Sos1.variables 24
        ConstraintHints.sos1_constraints;Sos1.variables;BTreeSet[overhead] 24
        ConstraintHints.sos1_constraints;Vec[overhead] 24
        "###);
    }
}
