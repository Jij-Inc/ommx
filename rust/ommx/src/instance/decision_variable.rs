use crate::{
    ATol, Bound, DecisionVariable, DecisionVariableError, DecisionVariableLabel, Instance, Kind,
    VariableID,
};

impl Instance {
    /// Get all unique decision variable names in this instance
    ///
    /// Returns a set of all unique variable names that have at least one named variable.
    /// Variables without names are not included.
    pub fn decision_variable_names(&self) -> std::collections::BTreeSet<String> {
        self.decision_variables
            .keys()
            .filter_map(|id| self.variable_labels().name(*id).map(|s| s.to_owned()))
            .collect()
    }

    /// Get a decision variable by name and subscripts
    ///
    /// # Arguments
    /// * `name` - The name of the decision variable to find
    /// * `subscripts` - The subscripts of the decision variable (can be empty)
    ///
    /// # Returns
    /// * `Some((VariableID, &DecisionVariable))` if a variable with the given name and subscripts is found
    /// * `None` if no matching variable is found
    ///
    /// # Example
    /// ```
    /// use ommx::Instance;
    ///
    /// let instance = Instance::default();
    /// // Find variable with name "x" and no subscripts
    /// let var = instance.get_decision_variable_by_name("x", vec![]);
    /// // Find variable with name "y" and subscripts [1, 2]
    /// let var_indexed = instance.get_decision_variable_by_name("y", vec![1, 2]);
    /// ```
    pub fn get_decision_variable_by_name(
        &self,
        name: &str,
        subscripts: Vec<i64>,
    ) -> Option<(VariableID, &DecisionVariable)> {
        let store = self.variable_labels();
        self.decision_variables.iter().find_map(|(id, var)| {
            (store.name(*id) == Some(name) && store.subscripts(*id) == subscripts.as_slice())
                .then_some((*id, var))
        })
    }

    /// Returns the next available [`VariableID`].
    ///
    /// Finds the maximum ID from decision variables, then adds 1.
    /// If there are no variables, returns `Ok(VariableID(0))`.
    ///
    /// Note: This method does not track which IDs have been allocated.
    /// Consecutive calls will return the same ID until a variable is actually added.
    ///
    /// # Errors
    ///
    /// Returns [`DecisionVariableError::NoAvailableID`] when the existing
    /// maximum ID is `u64::MAX` and no fresh ID can be allocated.
    pub fn next_variable_id(&self) -> Result<VariableID, DecisionVariableError> {
        self.decision_variables
            .last_key_value()
            .map(|(id, _)| {
                id.into_inner()
                    .checked_add(1)
                    .map(VariableID::from)
                    .ok_or(DecisionVariableError::NoAvailableID)
            })
            .unwrap_or(Ok(VariableID::from(0)))
    }

    pub(super) fn ensure_new_decision_variable_capacity(
        &self,
        count: usize,
    ) -> Result<(), DecisionVariableError> {
        if count == 0 {
            return Ok(());
        }
        let count = u64::try_from(count).map_err(|_| DecisionVariableError::NoAvailableID)?;
        if let Some((id, _)) = self.decision_variables.last_key_value() {
            id.into_inner()
                .checked_add(count)
                .ok_or(DecisionVariableError::NoAvailableID)?;
        }
        Ok(())
    }

    /// Create and atomically add a fully configured decision variable.
    ///
    /// The variable's `kind` and `bound` are normalized into a
    /// [`DecisionVariable`] row, while `label` and `fixed_value` are stored in
    /// the same [`crate::DecisionVariableTable`] entry under a newly allocated
    /// [`VariableID`]. `fixed_value`, when present, must satisfy the normalized
    /// kind and bound under `atol`.
    ///
    /// All validation completes before the instance is mutated. If this method
    /// returns an error, the decision-variable row, label store, fixed-value
    /// column, and next available ID are unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`DecisionVariableError::BoundInconsistentToKind`] when `bound`
    /// contains no value allowed by `kind`,
    /// [`DecisionVariableError::NoAvailableID`] when the ID space is exhausted,
    /// [`DecisionVariableError::NonFiniteValue`] when `fixed_value` is not
    /// finite, or [`DecisionVariableError::SubstitutedValueInconsistent`] when
    /// it does not belong to the normalized variable domain.
    pub fn new_decision_variable(
        &mut self,
        kind: Kind,
        bound: Bound,
        label: DecisionVariableLabel,
        fixed_value: Option<f64>,
        atol: ATol,
    ) -> Result<VariableID, DecisionVariableError> {
        let dv = DecisionVariable::new(kind, bound, atol)?;
        let id = self.next_variable_id()?;
        self.decision_variables
            .insert(id, dv, label, fixed_value, atol)?;
        Ok(id)
    }

    /// Create and atomically add a binary decision variable.
    ///
    /// `bound` is normalized to the allowed binary values. See
    /// [`Self::new_decision_variable`] for the insertion and error contract.
    pub fn new_binary(
        &mut self,
        bound: Bound,
        label: DecisionVariableLabel,
        fixed_value: Option<f64>,
        atol: ATol,
    ) -> Result<VariableID, DecisionVariableError> {
        self.new_decision_variable(Kind::Binary, bound, label, fixed_value, atol)
    }

    /// Create and atomically add an integer decision variable.
    ///
    /// `bound` is normalized inward to integer endpoints. See
    /// [`Self::new_decision_variable`] for the insertion and error contract.
    pub fn new_integer(
        &mut self,
        bound: Bound,
        label: DecisionVariableLabel,
        fixed_value: Option<f64>,
        atol: ATol,
    ) -> Result<VariableID, DecisionVariableError> {
        self.new_decision_variable(Kind::Integer, bound, label, fixed_value, atol)
    }

    /// Create and atomically add a continuous decision variable.
    ///
    /// See [`Self::new_decision_variable`] for the insertion and error contract.
    pub fn new_continuous(
        &mut self,
        bound: Bound,
        label: DecisionVariableLabel,
        fixed_value: Option<f64>,
        atol: ATol,
    ) -> Result<VariableID, DecisionVariableError> {
        self.new_decision_variable(Kind::Continuous, bound, label, fixed_value, atol)
    }

    /// Create and atomically add a semi-integer decision variable.
    ///
    /// `bound` is normalized inward to integer endpoints while preserving the
    /// semi-integer zero alternative. See [`Self::new_decision_variable`] for
    /// the insertion and error contract.
    pub fn new_semi_integer(
        &mut self,
        bound: Bound,
        label: DecisionVariableLabel,
        fixed_value: Option<f64>,
        atol: ATol,
    ) -> Result<VariableID, DecisionVariableError> {
        self.new_decision_variable(Kind::SemiInteger, bound, label, fixed_value, atol)
    }

    /// Create and atomically add a semi-continuous decision variable.
    ///
    /// See [`Self::new_decision_variable`] for the insertion and error contract.
    pub fn new_semi_continuous(
        &mut self,
        bound: Bound,
        label: DecisionVariableLabel,
        fixed_value: Option<f64>,
        atol: ATol,
    ) -> Result<VariableID, DecisionVariableError> {
        self.new_decision_variable(Kind::SemiContinuous, bound, label, fixed_value, atol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, Function, Sense};
    use maplit::btreemap;
    use std::collections::BTreeMap;

    #[test]
    fn test_next_variable_id() {
        // Empty instance should return 0
        let decision_variables = BTreeMap::new();
        let objective = coeff!(1.0).into();
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(instance.next_variable_id().unwrap(), VariableID::from(0));

        // Instance with variables should return max_id + 1
        let decision_variables = btreemap! {
            VariableID::from(5) => DecisionVariable::binary(),
            VariableID::from(8) => DecisionVariable::binary(),
            VariableID::from(100) => DecisionVariable::binary(),
        };
        let objective = (linear!(5) + coeff!(1.0)).into();
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        assert_eq!(instance.next_variable_id().unwrap(), VariableID::from(101));
    }

    #[test]
    fn test_next_variable_id_errors_when_id_space_is_exhausted() {
        let decision_variables = btreemap! {
            VariableID::from(u64::MAX) => DecisionVariable::binary(),
        };
        let instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        assert!(matches!(
            instance.next_variable_id(),
            Err(DecisionVariableError::NoAvailableID)
        ));
        assert!(matches!(
            instance.ensure_new_decision_variable_capacity(1),
            Err(DecisionVariableError::NoAvailableID)
        ));
    }

    #[test]
    fn test_next_variable_id_with_new_binary() {
        // Test integration with new_binary
        let decision_variables = BTreeMap::new();
        let objective = coeff!(1.0).into();
        let mut instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let var1 = instance
            .new_binary(
                Bound::of_binary(),
                DecisionVariableLabel::default(),
                None,
                ATol::default(),
            )
            .unwrap();
        assert_eq!(var1, VariableID::from(0));

        let var2 = instance
            .new_binary(
                Bound::of_binary(),
                DecisionVariableLabel::default(),
                None,
                ATol::default(),
            )
            .unwrap();
        assert_eq!(var2, VariableID::from(1));

        assert_eq!(instance.next_variable_id().unwrap(), VariableID::from(2));
    }

    #[test]
    fn new_decision_variable_commits_row_label_and_fixed_value_together() {
        let mut instance = Instance::default();
        let mut label = DecisionVariableLabel {
            name: Some("x".to_string()),
            subscripts: vec![1, 2],
            description: Some("bounded integer".to_string()),
            ..Default::default()
        };
        label
            .parameters
            .insert("region".to_string(), "east".to_string());

        let id = instance
            .new_integer(
                Bound::new(0.2, 3.8).unwrap(),
                label.clone(),
                Some(2.0),
                ATol::default(),
            )
            .unwrap();

        assert_eq!(id, VariableID::from(0));
        assert_eq!(
            instance.decision_variables()[&id].bound(),
            Bound::new(1.0, 3.0).unwrap()
        );
        assert_eq!(instance.variable_labels().collect_for(id), label);
        assert_eq!(instance.fixed_decision_variable_value(id), Some(2.0));
        assert_eq!(instance.next_variable_id().unwrap(), VariableID::from(1));
    }

    #[test]
    fn typed_new_decision_variables_set_kind_and_normalized_bound() {
        let mut instance = Instance::default();
        let atol = ATol::default();

        let binary = instance
            .new_binary(
                Bound::new(-1.0, 2.0).unwrap(),
                DecisionVariableLabel::default(),
                None,
                atol,
            )
            .unwrap();
        let integer = instance
            .new_integer(
                Bound::new(0.2, 3.8).unwrap(),
                DecisionVariableLabel::default(),
                None,
                atol,
            )
            .unwrap();
        let continuous = instance
            .new_continuous(
                Bound::new(0.2, 3.8).unwrap(),
                DecisionVariableLabel::default(),
                None,
                atol,
            )
            .unwrap();
        let semi_integer = instance
            .new_semi_integer(
                Bound::new(1.1, 1.9).unwrap(),
                DecisionVariableLabel::default(),
                None,
                atol,
            )
            .unwrap();
        let semi_continuous = instance
            .new_semi_continuous(
                Bound::new(0.5, 4.0).unwrap(),
                DecisionVariableLabel::default(),
                None,
                atol,
            )
            .unwrap();

        let variables = instance.decision_variables();
        assert_eq!(variables[&binary].kind(), Kind::Binary);
        assert_eq!(variables[&binary].bound(), Bound::of_binary());
        assert_eq!(variables[&integer].kind(), Kind::Integer);
        assert_eq!(variables[&integer].bound(), Bound::new(1.0, 3.0).unwrap());
        assert_eq!(variables[&continuous].kind(), Kind::Continuous);
        assert_eq!(
            variables[&continuous].bound(),
            Bound::new(0.2, 3.8).unwrap()
        );
        assert_eq!(variables[&semi_integer].kind(), Kind::SemiInteger);
        assert_eq!(
            variables[&semi_integer].bound(),
            Bound::new(0.0, 0.0).unwrap()
        );
        assert_eq!(variables[&semi_continuous].kind(), Kind::SemiContinuous);
        assert_eq!(
            variables[&semi_continuous].bound(),
            Bound::new(0.5, 4.0).unwrap()
        );
    }

    #[test]
    fn new_decision_variable_rejects_inconsistent_bound_atomically() {
        let mut instance = Instance::default();
        let before = instance.clone();

        let err = instance
            .new_integer(
                Bound::new(1.1, 1.9).unwrap(),
                DecisionVariableLabel {
                    name: Some("invalid".to_string()),
                    ..Default::default()
                },
                None,
                ATol::default(),
            )
            .unwrap_err();

        assert!(matches!(
            err,
            DecisionVariableError::BoundInconsistentToKind {
                kind: Kind::Integer,
                ..
            }
        ));
        assert_eq!(instance, before);
    }

    #[test]
    fn new_decision_variable_rejects_inconsistent_fixed_value_atomically() {
        let mut instance = Instance::default();
        let before = instance.clone();

        let err = instance
            .new_integer(
                Bound::new(0.0, 2.0).unwrap(),
                DecisionVariableLabel {
                    name: Some("invalid".to_string()),
                    ..Default::default()
                },
                Some(0.5),
                ATol::default(),
            )
            .unwrap_err();

        assert!(matches!(
            err,
            DecisionVariableError::SubstitutedValueInconsistent {
                id,
                substituted_value: 0.5,
                ..
            } if id == VariableID::from(0)
        ));
        assert_eq!(instance, before);
    }

    #[test]
    fn new_decision_variable_rejects_non_finite_fixed_value_atomically() {
        let mut instance = Instance::default();
        let before = instance.clone();

        let err = instance
            .new_continuous(
                Bound::default(),
                DecisionVariableLabel {
                    name: Some("invalid".to_string()),
                    ..Default::default()
                },
                Some(f64::NAN),
                ATol::default(),
            )
            .unwrap_err();

        assert!(matches!(
            err,
            DecisionVariableError::NonFiniteValue { id, value }
                if id == VariableID::from(0) && value.is_nan()
        ));
        assert_eq!(instance, before);
    }

    #[test]
    fn new_decision_variable_rejects_exhausted_id_atomically() {
        let decision_variables = btreemap! {
            VariableID::from(u64::MAX) => DecisionVariable::binary(),
        };
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();
        let before = instance.clone();

        let err = instance
            .new_binary(
                Bound::of_binary(),
                DecisionVariableLabel {
                    name: Some("overflow".to_string()),
                    ..Default::default()
                },
                None,
                ATol::default(),
            )
            .unwrap_err();

        assert!(matches!(err, DecisionVariableError::NoAvailableID));
        assert_eq!(instance, before);
    }
}
