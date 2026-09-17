//! Checked attachment of legacy hints to a retained regular formulation.

use super::{Instance, OneHotPromotionRequest, Sos1BigMPromotionRequest};
use crate::{v1, ConstraintID};
use std::collections::BTreeMap;

fn same_ids(actual: &[u64], expected: &[u64]) -> bool {
    let mut actual = actual.to_vec();
    actual.sort_unstable();
    actual == expected
}

impl Instance {
    /// Construct a V1 message with checked hints for existing regular rows.
    ///
    /// Hints are mutable, untrusted protobuf data, even when originally emitted
    /// by a promotion plan. Revalidate their membership, backing rows, selector
    /// isolation, and compatibility against this instance. The full ordinary
    /// formulation is retained so readers may safely ignore all hints.
    /// SOS1 validation may tighten variable bounds using the hinted Big-M links;
    /// these bounds are included in the returned V1 message.
    ///
    /// This operation does not apply promotion, lower special constraints,
    /// allocate auxiliary IDs, or reconstruct historical rows. The usual
    /// checked V1 conversion restrictions still apply. Empty hints produce the
    /// same message as ordinary conversion, without a hints field. Otherwise
    /// the supplied hint order is preserved; repeated identical hints are
    /// allowed, but conflicting claims and repeated IDs within a hint fail.
    ///
    /// # Errors
    ///
    /// Returns an error if any hint is invalid or the instance cannot be
    /// represented in V1. No partially validated message is returned.
    ///
    /// # Example
    ///
    /// ```
    /// use ommx::{Constraint, ConstraintID, DecisionVariable, Function, Instance,
    ///     Message, OneHotPromotionRequest, Sense, VariableID, coeff, linear, v1};
    /// use std::collections::BTreeMap;
    /// # fn example() -> ommx::Result<()> {
    /// let mut instance = Instance::new(
    ///     Sense::Minimize, Function::Zero,
    ///     BTreeMap::from([(VariableID::from(1), DecisionVariable::binary())]),
    ///     BTreeMap::from([(ConstraintID::from(7),
    ///         Constraint::equal_to_zero(Function::from((linear!(1) - coeff!(1.0))?)))]),
    /// )?;
    /// let request = OneHotPromotionRequest::from([ConstraintID::from(7)]);
    /// let hints = instance.plan_promote_one_hot(&request).into_v1_hints()?;
    /// let raw: v1::Instance = instance.into_v1_with_hints(hints)?;
    /// assert_eq!(raw.constraints[0].id, 7);
    /// let bytes = raw.encode_to_vec();
    /// # Ok(())
    /// # }
    /// ```
    pub fn into_v1_with_hints(
        mut self,
        additional_hints: v1::ConstraintHints,
    ) -> crate::Result<v1::Instance> {
        let one_hot_request = additional_hints
            .one_hot_constraints
            .iter()
            .map(|hint| ConstraintID::from(hint.constraint_id))
            .collect::<OneHotPromotionRequest>();
        let checked_one_hot = self
            .plan_promote_one_hot(&one_hot_request)
            .into_v1_hints()?
            .one_hot_constraints
            .into_iter()
            .map(|hint| (hint.constraint_id, hint.decision_variables))
            .collect::<BTreeMap<_, _>>();
        for hint in &additional_hints.one_hot_constraints {
            if !same_ids(
                &hint.decision_variables,
                &checked_one_hot[&hint.constraint_id],
            ) {
                crate::bail!(
                    { source_constraint_id = hint.constraint_id },
                    "V1 OneHot hint members do not match the active source equality"
                );
            }
        }

        let mut sos1_request = Sos1BigMPromotionRequest::new();
        for hint in &additional_hints.sos1_constraints {
            for (id, claims) in self.sos1_big_m_promotion_request_from_v1_hint(hint)? {
                if sos1_request
                    .get(&id)
                    .is_some_and(|previous| previous != &claims)
                {
                    crate::bail!(
                        { ?id },
                        "V1 SOS1 hints contain conflicting claims for cardinality constraint {id}"
                    );
                }
                sos1_request.insert(id, claims);
            }
        }
        let checked_sos1 = self
            .plan_promote_sos1_big_m(&sos1_request)
            .apply_bound_tightening_and_convert_to_v1_hints()?
            .sos1_constraints
            .into_iter()
            .map(|hint| (hint.binary_constraint_id, hint))
            .collect::<BTreeMap<_, _>>();
        for hint in &additional_hints.sos1_constraints {
            let checked = &checked_sos1[&hint.binary_constraint_id];
            if !same_ids(&hint.decision_variables, &checked.decision_variables)
                || !same_ids(&hint.big_m_constraint_ids, &checked.big_m_constraint_ids)
            {
                crate::bail!(
                    { cardinality_constraint_id = hint.binary_constraint_id },
                    "V1 SOS1 hint has repeated or inconsistent member or link IDs"
                );
            }
        }

        let mut raw = v1::Instance::try_from(self)?;
        if !additional_hints.one_hot_constraints.is_empty()
            || !additional_hints.sos1_constraints.is_empty()
        {
            raw.constraint_hints = Some(additional_hints);
        }
        Ok(raw)
    }
}
