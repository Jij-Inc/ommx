use super::*;
use crate::{ATol, Bounds};

impl Instance {
    /// Apply additional bounds to decision variables in the instance.
    ///
    /// This method clips the bounds of decision variables specified in the bounds map.
    /// All variable IDs in the bounds map must exist in the instance.
    ///
    /// If any operation fails, all changes are rolled back to maintain consistency.
    pub fn clip_bounds(&mut self, bounds: &Bounds, atol: ATol) -> anyhow::Result<()> {
        // Clone the current decision variables for potential rollback
        let backup = self.decision_variables.clone();

        // Attempt to apply all bound changes
        let result: anyhow::Result<()> = (|| {
            for (id, new_bound) in bounds {
                let decision_variable = self
                    .decision_variables
                    .get_mut(id)
                    .ok_or(InstanceError::UndefinedVariableID { id: *id })?;
                decision_variable.clip_bound(*new_bound, atol)?;
            }
            Ok(())
        })();

        // If any error occurred, rollback to the original state
        if result.is_err() {
            self.decision_variables = backup;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bound, DecisionVariable, VariableID};
    use std::collections::BTreeMap;

    #[test]
    fn test_clip_bounds_normal() {
        // Create instance with 3 variables
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );
        decision_variables.insert(
            VariableID::from(2),
            DecisionVariable::continuous(VariableID::from(2)),
        );
        decision_variables.insert(
            VariableID::from(3),
            DecisionVariable::continuous(VariableID::from(3)),
        );

        // Set initial bounds
        for dv in decision_variables.values_mut() {
            dv.set_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
                .unwrap();
        }

        let mut instance = Instance {
            decision_variables,
            ..Default::default()
        };

        // Apply new bounds to variables 1 and 2
        let mut new_bounds = Bounds::new();
        new_bounds.insert(VariableID::from(1), Bound::new(2.0, 8.0).unwrap());
        new_bounds.insert(VariableID::from(2), Bound::new(5.0, 15.0).unwrap());

        instance.clip_bounds(&new_bounds, ATol::default()).unwrap();

        // Check results
        assert_eq!(
            instance.decision_variables[&VariableID::from(1)].bound(),
            Bound::new(2.0, 8.0).unwrap()
        );
        assert_eq!(
            instance.decision_variables[&VariableID::from(2)].bound(),
            Bound::new(5.0, 10.0).unwrap() // Intersection of [0, 10] and [5, 15]
        );
        assert_eq!(
            instance.decision_variables[&VariableID::from(3)].bound(),
            Bound::new(0.0, 10.0).unwrap() // Unchanged
        );
    }

    #[test]
    fn test_clip_bounds_undefined_variable() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );

        let mut instance = Instance {
            decision_variables,
            ..Default::default()
        };

        // Try to clip bounds for non-existent variable
        let mut new_bounds = Bounds::new();
        new_bounds.insert(VariableID::from(999), Bound::new(0.0, 1.0).unwrap());

        let result = instance.clip_bounds(&new_bounds, ATol::default());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("999"));
    }

    #[test]
    fn test_clip_bounds_rollback() {
        // Create instance with 3 variables
        let mut decision_variables = BTreeMap::new();
        for i in 1..=3 {
            let mut dv = DecisionVariable::continuous(VariableID::from(i));
            dv.set_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
                .unwrap();
            decision_variables.insert(VariableID::from(i), dv);
        }

        let mut instance = Instance {
            decision_variables,
            ..Default::default()
        };

        // Store original bounds for verification
        let original_bounds: Vec<_> = instance
            .decision_variables
            .values()
            .map(|dv| dv.bound())
            .collect();

        // Apply changes where the second one will cause an empty intersection error
        let mut new_bounds = Bounds::new();
        new_bounds.insert(VariableID::from(1), Bound::new(2.0, 8.0).unwrap());
        new_bounds.insert(VariableID::from(2), Bound::new(15.0, 20.0).unwrap()); // No intersection with [0, 10]
        new_bounds.insert(VariableID::from(3), Bound::new(3.0, 7.0).unwrap());

        let result = instance.clip_bounds(&new_bounds, ATol::default());
        assert!(result.is_err());

        // Verify all bounds were rolled back to original values
        let current_bounds: Vec<_> = instance
            .decision_variables
            .values()
            .map(|dv| dv.bound())
            .collect();
        assert_eq!(original_bounds, current_bounds);
    }

    #[test]
    fn test_clip_bounds_empty() {
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(
            VariableID::from(1),
            DecisionVariable::continuous(VariableID::from(1)),
        );

        let mut instance = Instance {
            decision_variables,
            ..Default::default()
        };

        // Apply empty bounds map (should succeed and change nothing)
        let new_bounds = Bounds::new();
        instance.clip_bounds(&new_bounds, ATol::default()).unwrap();
    }
}
