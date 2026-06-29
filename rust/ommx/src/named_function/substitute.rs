use super::*;
use crate::{substitute_one_via_acyclic, Function, Substitute, SubstitutionError, VariableID};

impl Substitute for NamedFunction {
    type Output = Self;

    fn substitute_acyclic(
        mut self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, SubstitutionError> {
        self.function = self.function.substitute_acyclic(acyclic)?;
        Ok(self)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        substitute_one_via_acyclic(self, assigned, f)
    }
}

impl Substitute for NamedFunctionTable<NamedFunction> {
    type Output = Self;

    fn substitute_acyclic(
        self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, SubstitutionError> {
        let NamedFunctionTable { entries, labels } = self;
        let mut substituted_entries = std::collections::BTreeMap::new();
        for (id, named_function) in entries {
            let substituted = named_function
                .substitute_acyclic(acyclic)
                .inspect_err(|e| {
                    tracing::error!(?id, error = %e, "failed to substitute named function");
                })?;
            substituted_entries.insert(id, substituted);
        }
        Ok(NamedFunctionTable {
            entries: substituted_entries,
            labels,
        })
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        f: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        substitute_one_via_acyclic(self, assigned, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, Evaluate};
    use maplit::btreeset;

    #[test]
    fn test_named_function_table_substitute_preserves_labels() {
        let id = NamedFunctionID::from(1);
        let mut entries = std::collections::BTreeMap::new();
        entries.insert(
            id,
            NamedFunction {
                function: Function::from((linear!(1) + linear!(2)).unwrap()),
            },
        );
        let mut labels = NamedFunctionLabelStore::new();
        labels.set_name(id, "tracked");
        let table = NamedFunctionTable::new(entries, labels).unwrap();

        let substituted = table
            .substitute_one(
                VariableID::from(1),
                &Function::from((linear!(3) + coeff!(1.0)).unwrap()),
            )
            .unwrap();

        assert_eq!(substituted.labels().name(id), Some("tracked"));
        assert_eq!(
            substituted.get(&id).unwrap().required_ids(),
            btreeset! { VariableID::from(2), VariableID::from(3) }
        );
    }
}
