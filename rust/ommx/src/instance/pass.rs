use super::*;
use anyhow::{anyhow, Result};

impl Instance {
    pub fn relax(
        &mut self,
        id: ConstraintID,
        removed_reason: String,
        parameters: impl IntoIterator<Item = (String, String)>,
    ) -> Result<()> {
        let c = self
            .constraints
            .remove(&id)
            .ok_or_else(|| anyhow!("Constraint with ID {:?} not found", id))?;
        self.removed_constraints.insert(
            id,
            RemovedConstraint {
                constraint: c,
                removed_reason,
                removed_reason_parameters: parameters.into_iter().collect(),
            },
        );
        Ok(())
    }

    pub fn restore(&mut self, id: ConstraintID) -> Result<()> {
        let rc = self
            .removed_constraints
            .remove(&id)
            .ok_or_else(|| anyhow!("Removed constraint with ID {:?} not found", id))?;
        self.constraints.insert(id, rc.constraint);
        Ok(())
    }
}
