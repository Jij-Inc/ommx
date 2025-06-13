use crate::{ATol, Bound, DecisionVariable, DecisionVariableError, Instance, Kind, VariableID};

impl Instance {
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
