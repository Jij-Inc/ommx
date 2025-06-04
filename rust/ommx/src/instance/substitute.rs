use super::*;
use crate::{Function, Substitute, SubstitutionError, VariableID};

impl Substitute for Instance {
    type Output = Self;

    fn substitute_one(
        mut self,
        assigned: VariableID,
        function: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        // Apply substitution to the objective function
        self.objective = self.objective.clone().substitute_one(assigned, function)?;

        // Apply substitution to all constraints
        for constraint in self.constraints.values_mut() {
            constraint.function = constraint
                .function
                .clone()
                .substitute_one(assigned, function)?;
        }

        // Apply substitution to all removed constraints
        for removed_constraint in self.removed_constraints.values_mut() {
            removed_constraint.constraint.function = removed_constraint
                .constraint
                .function
                .clone()
                .substitute_one(assigned, function)?;
        }

        // Apply substitution to the existing decision_variable_dependency
        self.decision_variable_dependency = self
            .decision_variable_dependency
            .clone()
            .substitute_one(assigned, function)?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, constraint::Equality, linear, DecisionVariable, Sense};
    use std::collections::BTreeMap;

    #[test]
    fn test_instance_substitute() {
        // Create decision variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::continuous(VariableID::from(2)),
        );

        // Create a simple instance: minimize x1 + 2*x2, subject to x1 + x2 <= 10
        let objective = Function::from(linear!(1) + coeff!(2.0) * linear!(2));
        let constraint_function = Function::from(linear!(1) + linear!(2) + coeff!(-10.0));

        let mut constraints = BTreeMap::new();
        let constraint = Constraint {
            id: ConstraintID::from(1),
            function: constraint_function,
            equality: Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: Vec::new(),
            parameters: Default::default(),
            description: None,
        };
        constraints.insert(ConstraintID::from(1), constraint);
        let constraint_hints = ConstraintHints::default();

        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            constraints,
            constraint_hints,
        )
        .unwrap();

        // Substitute x1 with x3 + 1
        let substitution = Function::from(linear!(3) + coeff!(1.0));
        let result = instance
            .substitute_one(VariableID::from(1), &substitution)
            .unwrap();

        // Check that the decision_variable_dependency contains the assignment x1 <- x3 + 1
        assert_eq!(result.decision_variable_dependency.len(), 1);
        assert!(result
            .decision_variable_dependency
            .get(&VariableID::from(1))
            .is_some());
    }
}
