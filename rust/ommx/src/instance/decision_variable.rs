use crate::{ATol, Bound, DecisionVariable, DecisionVariableError, Instance, Kind, VariableID};

impl Instance {
    /// Get all unique decision variable names in this instance
    ///
    /// Returns a set of all unique variable names that have at least one named variable.
    /// Variables without names are not included.
    pub fn decision_variable_names(&self) -> std::collections::BTreeSet<String> {
        self.decision_variables
            .values()
            .filter_map(|var| var.metadata.name.clone())
            .collect()
    }

    /// Get a decision variable by name and subscripts
    ///
    /// # Arguments
    /// * `name` - The name of the decision variable to find
    /// * `subscripts` - The subscripts of the decision variable (can be empty)
    ///
    /// # Returns
    /// * `Some(&DecisionVariable)` if a variable with the given name and subscripts is found
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
    ) -> Option<&DecisionVariable> {
        self.decision_variables.values().find(|var| {
            var.metadata.name.as_deref() == Some(name) && var.metadata.subscripts == subscripts
        })
    }
    /// Returns the next available VariableID.
    ///
    /// Finds the maximum ID from decision variables, then adds 1.
    /// If there are no variables, returns VariableID(0).
    ///
    /// Note: This method does not track which IDs have been allocated.
    /// Consecutive calls will return the same ID until a variable is actually added.
    pub fn next_variable_id(&self) -> VariableID {
        self.decision_variables
            .last_key_value()
            .map(|(id, _)| VariableID::from(id.into_inner() + 1))
            .unwrap_or(VariableID::from(0))
    }

    pub fn new_decision_variable(
        &mut self,
        kind: Kind,
        bound: Bound,
        substituted_value: Option<f64>,
        atol: ATol,
    ) -> Result<&mut DecisionVariable, DecisionVariableError> {
        let id = self.next_variable_id();
        let dv = DecisionVariable::new(id, kind, bound, substituted_value, atol)?;
        self.decision_variables.insert(id, dv);
        Ok(self.decision_variables.get_mut(&id).unwrap())
    }

    pub fn new_binary(&mut self) -> &mut DecisionVariable {
        self.new_decision_variable(Kind::Binary, Bound::of_binary(), None, ATol::default())
            .unwrap()
    }

    pub fn new_integer(&mut self) -> &mut DecisionVariable {
        self.new_decision_variable(Kind::Integer, Bound::default(), None, ATol::default())
            .unwrap()
    }

    pub fn new_continuous(&mut self) -> &mut DecisionVariable {
        self.new_decision_variable(Kind::Continuous, Bound::default(), None, ATol::default())
            .unwrap()
    }

    pub fn new_semi_integer(&mut self) -> &mut DecisionVariable {
        self.new_decision_variable(Kind::SemiInteger, Bound::default(), None, ATol::default())
            .unwrap()
    }

    pub fn new_semi_continuous(&mut self) -> &mut DecisionVariable {
        self.new_decision_variable(
            Kind::SemiContinuous,
            Bound::default(),
            None,
            ATol::default(),
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, Sense};
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
        assert_eq!(instance.next_variable_id(), VariableID::from(0));

        // Instance with variables should return max_id + 1
        let decision_variables = btreemap! {
            VariableID::from(5) => DecisionVariable::binary(VariableID::from(5)),
            VariableID::from(8) => DecisionVariable::binary(VariableID::from(8)),
            VariableID::from(100) => DecisionVariable::binary(VariableID::from(100)),
        };
        let objective = (linear!(5) + coeff!(1.0)).into();
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        assert_eq!(instance.next_variable_id(), VariableID::from(101));
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

        let var1 = instance.new_binary();
        assert_eq!(var1.id(), VariableID::from(0));

        let var2 = instance.new_binary();
        assert_eq!(var2.id(), VariableID::from(1));

        assert_eq!(instance.next_variable_id(), VariableID::from(2));
    }
}
