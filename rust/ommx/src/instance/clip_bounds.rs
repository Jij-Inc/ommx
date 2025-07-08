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
