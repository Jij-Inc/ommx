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
        // Track original bounds for potential rollback
        let mut original_bounds = BTreeMap::new();

        // Attempt to apply all bound changes
        let result: anyhow::Result<()> = (|| {
            for (id, new_bound) in bounds {
                let decision_variable = self
                    .decision_variables
                    .get_mut(id)
                    .ok_or(InstanceError::UndefinedVariableID { id: *id })?;

                // Store original bound only if it actually changes
                let original_bound = decision_variable.bound();
                let changed = decision_variable.clip_bound(*new_bound, atol)?;

                if changed {
                    original_bounds.insert(*id, original_bound);
                }
            }
            Ok(())
        })();

        // If any error occurred, rollback only the modified entries
        if result.is_err() {
            for (id, original_bound) in original_bounds {
                if let Some(dv) = self.decision_variables.get_mut(&id) {
                    // Safe to unwrap because we're restoring a previously valid bound
                    dv.set_bound(original_bound, atol).unwrap();
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bound, DecisionVariable, VariableID};
    use maplit::btreemap;

    #[test]
    fn test_clip_bounds_normal() {
        // Create instance with 3 variables
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1))
                .with_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
                .unwrap(),
            VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2))
                .with_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
                .unwrap(),
            VariableID::from(3) => DecisionVariable::continuous(VariableID::from(3))
                .with_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
                .unwrap(),
        };

        let mut instance = Instance {
            decision_variables,
            ..Default::default()
        };

        // Apply new bounds to variables 1 and 2
        let new_bounds = btreemap! {
            VariableID::from(1) => Bound::new(2.0, 8.0).unwrap(),
            VariableID::from(2) => Bound::new(5.0, 15.0).unwrap(),
        };

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
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
        };

        let mut instance = Instance {
            decision_variables,
            ..Default::default()
        };

        // Try to clip bounds for non-existent variable
        let new_bounds = btreemap! {
            VariableID::from(999) => Bound::new(0.0, 1.0).unwrap(),
        };

        let result = instance.clip_bounds(&new_bounds, ATol::default());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("999"));
    }

    #[test]
    fn test_clip_bounds_rollback() {
        // Create instance with 3 variables
        let mut decision_variables = btreemap! {};
        for i in 1..=3 {
            let dv = DecisionVariable::continuous(VariableID::from(i))
                .with_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
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
        let new_bounds = btreemap! {
            VariableID::from(1) => Bound::new(2.0, 8.0).unwrap(),
            VariableID::from(2) => Bound::new(15.0, 20.0).unwrap(), // No intersection with [0, 10]
            VariableID::from(3) => Bound::new(3.0, 7.0).unwrap(),
        };

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
        let dv = DecisionVariable::continuous(VariableID::from(1))
            .with_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
            .unwrap();
        let original_bound = dv.bound();

        let decision_variables = btreemap! {
            VariableID::from(1) => dv,
        };

        let mut instance = Instance {
            decision_variables,
            ..Default::default()
        };

        // Apply empty bounds map (should succeed and change nothing)
        let new_bounds = btreemap! {};
        instance.clip_bounds(&new_bounds, ATol::default()).unwrap();

        // Assert that the bound remains unchanged
        assert_eq!(
            instance.decision_variables[&VariableID::from(1)].bound(),
            original_bound
        );
    }
}
