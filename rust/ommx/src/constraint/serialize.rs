use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Constraint<Created> {
    /// Serialize this constraint to bytes with an explicit [`ConstraintID`].
    ///
    /// Because `Constraint` does not own an ID, callers must supply the ID that
    /// identifies this constraint within its enclosing collection.
    pub fn to_bytes(&self, id: ConstraintID) -> Vec<u8> {
        let v1_constraint = v1::Constraint::from((id, self.clone()));
        v1_constraint.encode_to_vec()
    }

    /// Deserialize bytes into a `(ConstraintID, Constraint)` pair.
    ///
    /// Per-element parsing now also yields a `ConstraintMetadata`; for the
    /// standalone `from_bytes` API we drop it (the v3 metadata redesign
    /// keeps metadata at the collection layer, so a bare per-element
    /// round-trip cannot preserve it).
    pub fn from_bytes(bytes: &[u8]) -> Result<(ConstraintID, Self)> {
        let inner = v1::Constraint::decode(bytes)?;
        let (id, c, _metadata) = Parse::parse(inner, &())?;
        Ok((id, c))
    }
}

impl EvaluatedConstraint {
    /// Serialize this evaluated constraint to bytes with an explicit [`ConstraintID`].
    pub fn to_bytes(&self, id: ConstraintID) -> Vec<u8> {
        let v1_evaluated_constraint = v1::EvaluatedConstraint::from((id, self.clone()));
        v1_evaluated_constraint.encode_to_vec()
    }

    /// Deserialize bytes into a `(ConstraintID, EvaluatedConstraint)` pair.
    pub fn from_bytes(bytes: &[u8]) -> Result<(ConstraintID, Self)> {
        let inner = v1::EvaluatedConstraint::decode(bytes)?;
        let (id, constraint, _metadata, _removed_reason) = Parse::parse(inner, &())?;
        Ok((id, constraint))
    }
}

impl SampledConstraint {
    /// Serialize this sampled constraint to bytes with an explicit [`ConstraintID`].
    pub fn to_bytes(&self, id: ConstraintID) -> Vec<u8> {
        let v1_sampled_constraint = v1::SampledConstraint::from((id, self.clone()));
        v1_sampled_constraint.encode_to_vec()
    }

    /// Deserialize bytes into a `(ConstraintID, SampledConstraint)` pair.
    pub fn from_bytes(bytes: &[u8]) -> Result<(ConstraintID, Self)> {
        let inner = v1::SampledConstraint::decode(bytes)?;
        let (id, constraint, _metadata, _removed_reason) = Parse::parse(inner, &())?;
        Ok((id, constraint))
    }
}
