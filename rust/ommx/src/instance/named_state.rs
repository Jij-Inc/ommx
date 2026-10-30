use super::Instance;
use crate::{v1::State, Result, VariableID};
use std::collections::BTreeMap;

/// Decision-variable values grouped by modeling name and subscripts.
///
/// Scalars use an empty subscript vector. Keys are resolved against a target
/// [`Instance`] by [`Instance::state_from_named_values`]; they need not cover
/// every variable. Label parameters and descriptions are not part of the key.
pub type NamedState = BTreeMap<String, BTreeMap<Vec<i64>, f64>>;

/// A supplied name and subscript key cannot identify exactly one variable.
///
/// Correct or remove an unknown key, or use ID-based state entries when a
/// name/subscript pair is ambiguous. Conversion leaves the instance unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NamedStateError {
    /// No variable in the target instance has this name and these subscripts.
    #[error("No decision variable with name {name:?} and subscripts {subscripts:?}; correct or remove this key")]
    UnknownVariable { name: String, subscripts: Vec<i64> },
    /// Several variables have the requested label; use their IDs to distinguish them.
    #[error("Decision variables {variable_ids:?} share name {name:?} and subscripts {subscripts:?}; use an ID-based state to distinguish them")]
    AmbiguousVariable {
        name: String,
        subscripts: Vec<i64>,
        variable_ids: Vec<VariableID>,
    },
}

impl Instance {
    /// Resolve named decision-variable values into an ID-keyed state.
    ///
    /// Only supplied entries are included: omitted variables are not filled or
    /// inferred. Values are copied without checking finiteness, kind, bounds,
    /// fixed values, or dependencies; use evaluation for those checks. Negative
    /// zero is normalized to positive zero, as in Python's `State` constructor.
    /// Labels without a name cannot be addressed. Parameters and descriptions
    /// are ignored, so they cannot disambiguate a name/subscript pair.
    ///
    /// # Errors
    ///
    /// Returns [`NamedStateError`] if a supplied key matches zero or multiple
    /// variables. Ambiguous labels that are not supplied do not cause errors.
    /// Neither the instance nor the input is modified.
    ///
    /// ```
    /// use ommx::{ATol, Bound, DecisionVariableLabel, Instance, NamedState};
    /// let mut instance = Instance::default();
    /// let id = instance.new_binary(Bound::of_binary(), DecisionVariableLabel {
    ///     name: Some("x".into()), subscripts: vec![2], ..Default::default()
    /// }, None, ATol::default())?;
    /// let values = NamedState::from([("x".into(), [(vec![2], 1.0)].into())]);
    /// let state = instance.state_from_named_values(&values)?;
    /// assert_eq!(state.entries[&id.into_inner()], 1.0);
    /// # Ok::<(), ommx::Error>(())
    /// ```
    pub fn state_from_named_values(&self, values: &NamedState) -> Result<State> {
        let labels = self.variable_labels();
        let mut ids_by_label: BTreeMap<(&str, &[i64]), Vec<VariableID>> = BTreeMap::new();
        for id in self.decision_variables().keys() {
            if let Some(name) = labels.name(*id) {
                ids_by_label
                    .entry((name, labels.subscripts(*id)))
                    .or_default()
                    .push(*id);
            }
        }

        let mut entries = std::collections::HashMap::new();
        for (name, variables) in values {
            for (subscripts, value) in variables {
                let ids = ids_by_label.get(&(name.as_str(), subscripts.as_slice()));
                let id = match ids.map(Vec::as_slice) {
                    Some([id]) => *id,
                    Some(ids) => {
                        return Err(NamedStateError::AmbiguousVariable {
                            name: name.clone(),
                            subscripts: subscripts.clone(),
                            variable_ids: ids.to_vec(),
                        }
                        .into());
                    }
                    None => {
                        return Err(NamedStateError::UnknownVariable {
                            name: name.clone(),
                            subscripts: subscripts.clone(),
                        }
                        .into());
                    }
                };
                entries.insert(id.into_inner(), if *value == 0.0 { 0.0 } else { *value });
            }
        }
        Ok(State { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ATol, Bound, DecisionVariableLabel};

    #[test]
    fn resolves_labels_using_target_ids_and_preserves_partial_state() {
        let mut instance = Instance::default();
        instance
            .new_binary(
                Bound::of_binary(),
                DecisionVariableLabel::default(),
                None,
                ATol::default(),
            )
            .unwrap();
        let x = instance
            .new_binary(
                Bound::of_binary(),
                DecisionVariableLabel {
                    name: Some("x".into()),
                    subscripts: vec![-1, 2],
                    ..Default::default()
                },
                None,
                ATol::default(),
            )
            .unwrap();
        let scalar = instance
            .new_binary(
                Bound::of_binary(),
                DecisionVariableLabel {
                    name: Some("s".into()),
                    ..Default::default()
                },
                None,
                ATol::default(),
            )
            .unwrap();
        let values = NamedState::from([
            ("x".into(), [(vec![-1, 2], 3.5)].into()),
            ("s".into(), [(vec![], -0.0)].into()),
        ]);
        let state = instance.state_from_named_values(&values).unwrap();
        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.entries[&x.into_inner()], 3.5);
        assert_eq!(
            state.entries[&scalar.into_inner()].to_bits(),
            0.0_f64.to_bits()
        );
        assert!(instance
            .state_from_named_values(&NamedState::new())
            .unwrap()
            .entries
            .is_empty());
    }

    #[test]
    fn errors_expose_unknown_or_ambiguous_keys_without_mutation() {
        let mut instance = Instance::default();
        let label = DecisionVariableLabel {
            name: Some("x".into()),
            subscripts: vec![0],
            ..Default::default()
        };
        let a = instance
            .new_binary(Bound::of_binary(), label.clone(), None, ATol::default())
            .unwrap();
        let b = instance
            .new_binary(Bound::of_binary(), label, None, ATol::default())
            .unwrap();
        let before = instance.clone();
        for (key, expected) in [
            (
                vec![1],
                NamedStateError::UnknownVariable {
                    name: "x".into(),
                    subscripts: vec![1],
                },
            ),
            (
                vec![0],
                NamedStateError::AmbiguousVariable {
                    name: "x".into(),
                    subscripts: vec![0],
                    variable_ids: vec![a, b],
                },
            ),
        ] {
            let values = NamedState::from([("x".into(), [(key, 1.0)].into())]);
            let error = instance.state_from_named_values(&values).unwrap_err();
            assert_eq!(error.downcast_ref::<NamedStateError>(), Some(&expected));
            assert_eq!(instance, before);
        }
        assert!(instance.state_from_named_values(&NamedState::new()).is_ok());
    }
}
