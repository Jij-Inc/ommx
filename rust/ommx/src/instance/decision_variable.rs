use crate::{ATol, Bound, DecisionVariable, DecisionVariableError, Instance, Kind, VariableID};

impl Instance {
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
    fn next_variable_id(&mut self) -> VariableID {
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
