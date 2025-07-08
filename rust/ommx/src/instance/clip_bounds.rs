use super::*;
use crate::{ATol, Bounds};

impl Instance {
    /// Apply additional bounds to decision variables in the instance.
    ///
    /// This method clips the bounds of decision variables specified in the bounds map.
    /// All variable IDs in the bounds map must exist in the instance.
    pub fn clip_bounds(&mut self, bounds: &Bounds, atol: ATol) -> anyhow::Result<()> {
        for (id, new_bound) in bounds {
            let decision_variable = self
                .decision_variables
                .get_mut(id)
                .ok_or(InstanceError::UndefinedVariableID { id: *id })?;
            decision_variable.clip_bound(*new_bound, atol)?;
        }
        Ok(())
    }
}
