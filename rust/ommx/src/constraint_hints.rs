//! Internal types for parsing `v1::ConstraintHints` from proto format.
//!
//! These types are used only during deserialization to convert the legacy
//! `ConstraintHints` proto message into first-class `OneHotConstraint` and
//! `Sos1Constraint` collections. They are not part of the public API.

mod one_hot;
mod sos1;

pub use one_hot::OneHot;
pub use sos1::Sos1;

use crate::{
    constraint::RemovedReason,
    parse::{Parse, ParseError},
    v1, Constraint, ConstraintID, DecisionVariable, VariableID,
};
use std::collections::BTreeMap;

/// Internal representation of parsed constraint hints.
///
/// Used only as an intermediate during Instance deserialization.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConstraintHints {
    pub one_hot_constraints: Vec<OneHot>,
    pub sos1_constraints: Vec<Sos1>,
}

impl Parse for v1::ConstraintHints {
    type Output = ConstraintHints;
    type Context = (
        BTreeMap<VariableID, DecisionVariable>,
        BTreeMap<ConstraintID, Constraint>,
        BTreeMap<ConstraintID, (Constraint, RemovedReason)>,
    );
    fn parse(self, context: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.ConstraintHints";
        let (_, constraints, removed_constraints) = context;

        // Parse all hints first
        let one_hot_constraints: Vec<OneHot> = self
            .one_hot_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "one_hot_constraints"))
            .collect::<Result<Vec<_>, ParseError>>()?;
        let sos1_constraints: Vec<Sos1> = self
            .sos1_constraints
            .into_iter()
            .map(|c| c.parse_as(context, message, "sos1_constraints"))
            .collect::<Result<_, ParseError>>()?;

        // Filter out hints that reference removed or unknown constraints.
        // This is intentional healing behavior for deserialization: old serialized instances
        // may contain hints referencing constraints that have since been removed.
        // We silently discard such hints (with debug log) rather than failing.
        let one_hot_constraints: Vec<OneHot> = one_hot_constraints
            .into_iter()
            .filter(|hint| {
                if removed_constraints.contains_key(&hint.id) {
                    log::debug!(
                        "Discarding OneHot hint referencing removed constraint (id={:?})",
                        hint.id
                    );
                    false
                } else if !constraints.contains_key(&hint.id) {
                    log::debug!(
                        "Discarding OneHot hint referencing unknown constraint (id={:?})",
                        hint.id
                    );
                    false
                } else {
                    true
                }
            })
            .collect();

        let sos1_constraints: Vec<Sos1> = sos1_constraints
            .into_iter()
            .filter(|hint| {
                let binary_removed = removed_constraints.contains_key(&hint.binary_constraint_id);
                let big_m_removed = hint
                    .big_m_constraint_ids
                    .iter()
                    .any(|id| removed_constraints.contains_key(id));

                if binary_removed || big_m_removed {
                    log::debug!(
                        "Discarding Sos1 hint referencing removed constraint (binary_constraint_id={:?}, big_m_constraint_ids={:?})",
                        hint.binary_constraint_id,
                        hint.big_m_constraint_ids
                    );
                    false
                } else {
                    true
                }
            })
            .collect();

        Ok(ConstraintHints {
            one_hot_constraints,
            sos1_constraints,
        })
    }
}
