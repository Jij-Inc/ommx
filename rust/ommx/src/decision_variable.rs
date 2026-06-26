mod approx;
mod arbitrary;
mod label_store;
mod logical_memory;
pub(crate) mod parse;

pub use arbitrary::*;
use getset::CopyGetters;
pub use label_store::VariableLabelStore;

pub(crate) use parse::sampled_decision_variable_to_v1;

use crate::logical_memory::LogicalMemoryProfile;
use crate::{ATol, Bound, Parse, RawParseError, SampleID, Sampled};
use ::approx::AbsDiffEq;
use derive_more::{Deref, From};
use getset::Getters;
use std::collections::BTreeSet;

/// ID for decision variable and parameter.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Deref,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct VariableID(u64);
pub type VariableIDSet = BTreeSet<VariableID>;

impl VariableID {
    pub fn into_inner(&self) -> u64 {
        self.0
    }
}

impl From<VariableID> for u64 {
    fn from(id: VariableID) -> Self {
        id.0
    }
}

impl std::fmt::Debug for VariableID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VariableID({})", self.0)
    }
}

impl std::fmt::Display for VariableID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Kind {
    Continuous,
    Integer,
    Binary,
    SemiContinuous,
    SemiInteger,
}

impl Kind {
    /// Check and convert the bound to a consistent bound
    ///
    /// - For [`Kind::Continuous`] or [`Kind::SemiContinuous`], arbitrary bound is allowed.
    /// - For [`Kind::Integer`] or [`Kind::Binary`], the bound is restricted to integer or binary.
    ///   If there is no integer or binary in the bound, [`None`] is returned.
    /// - For [`Kind::SemiInteger`], the bound is also restricted to integer.
    ///   If there is no integer in the bound, on the other hand, returns `[0.0, 0.0]`.
    ///
    /// As a result, the returned bound, except for `None` case, is guaranteed that there is at least one possible value.
    ///
    /// Example
    /// --------
    ///
    /// ```rust
    /// use ommx::{Kind, Bound, ATol};
    ///
    /// // Any bound is allowed for Kind::Continuous
    /// assert_eq!(
    ///     Kind::Continuous.consistent_bound(Bound::new(1.0, 2.0).unwrap(), ATol::default()),
    ///     Some(Bound::new(1.0, 2.0).unwrap())
    /// );
    ///
    /// // For Kind::Integer, the bound is restricted to integer.
    /// assert_eq!(
    ///    Kind::Integer.consistent_bound(Bound::new(1.1, 2.9).unwrap(), ATol::default()),
    ///    Some(Bound::new(2.0, 2.0).unwrap())
    /// );
    ///
    /// // And if there is no integer in the bound, None is returned.
    /// assert_eq!(
    ///     Kind::Integer.consistent_bound(Bound::new(1.1, 1.9).unwrap(), ATol::default()),
    ///     None
    /// );
    ///
    /// // For Kind::Binary, there are only three acceptable bounds: [0, 0], [0, 1], [1, 1].
    /// assert_eq!(
    ///     Kind::Binary.consistent_bound(Bound::negative(), ATol::default()),
    ///     Some(Bound::new(0.0, 0.0).unwrap())
    /// );
    /// assert_eq!(
    ///     Kind::Binary.consistent_bound(Bound::new(0.5, f64::INFINITY).unwrap(), ATol::default()),
    ///     Some(Bound::new(1.0, 1.0).unwrap())
    /// );
    /// assert_eq!(
    ///    Kind::Binary.consistent_bound(Bound::default(), ATol::default()),
    ///    Some(Bound::new(0.0, 1.0).unwrap())
    /// );
    ///
    /// ```
    pub fn consistent_bound(&self, bound: Bound, atol: ATol) -> Option<Bound> {
        match self {
            Kind::Continuous | Kind::SemiContinuous => Some(bound),
            Kind::Integer => bound.as_integer_bound(atol),
            Kind::SemiInteger => Some(
                bound
                    .as_integer_bound(atol)
                    .unwrap_or_else(|| Bound::new(0.0, 0.0).unwrap()),
            ),
            Kind::Binary => {
                // Acceptable bounds are only [0, 0], [0, 1], [1, 1]
                if bound.lower() > 1.0 + atol || bound.upper() < 0.0 - atol {
                    return None;
                }
                if bound.lower() > 0.0 + atol {
                    Some(Bound::new(1.0, 1.0).unwrap())
                } else if bound.upper() < 1.0 - atol {
                    Some(Bound::new(0.0, 0.0).unwrap())
                } else {
                    Some(Bound::new(0.0, 1.0).unwrap())
                }
            }
        }
    }
}

fn ensure_finite_value(id: VariableID, value: f64) -> Result<(), DecisionVariableError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(DecisionVariableError::NonFiniteValue { id, value })
    }
}

/// Row data for a decision variable table.
///
/// Holds only `kind` and `bound` as its intrinsic definition. The
/// [`VariableID`] is owned by the enclosing decision-variable table key.
/// Auxiliary modeling label (`name`, `subscripts`, `parameters`,
/// `description`) lives on the enclosing [`Instance`](crate::Instance)'s
/// [`VariableLabelStore`](crate::VariableLabelStore) keyed by
/// [`VariableID`]; per-element label storage was retired in the v3
/// redesign.
///
/// Invariants
/// ----------
/// - `kind` and `bound` are consistent
///   - i.e. `bound` is invariant under `|bound| kind.consistent_bound(bound, atol).unwrap()` for appropriate `atol`.
///
#[derive(Debug, Clone, PartialEq, CopyGetters, LogicalMemoryProfile)]
pub struct DecisionVariable {
    #[getset(get_copy = "pub")]
    kind: Kind,
    #[getset(get_copy = "pub")]
    bound: Bound,
}

impl DecisionVariable {
    /// Create a new decision variable.
    pub fn new(kind: Kind, bound: Bound, atol: ATol) -> Result<Self, DecisionVariableError> {
        Ok(Self {
            kind,
            bound: kind
                .consistent_bound(bound, atol)
                .ok_or(DecisionVariableError::BoundInconsistentToKind { kind, bound })?,
        })
    }

    pub fn binary() -> Self {
        Self::new(Kind::Binary, Bound::of_binary(), ATol::default()).unwrap()
    }

    /// Unbounded integer decision variable.
    pub fn integer() -> Self {
        Self::new(Kind::Integer, Bound::default(), ATol::default()).unwrap()
    }

    /// Unbounded continuous decision variable.
    pub fn continuous() -> Self {
        Self::new(Kind::Continuous, Bound::default(), ATol::default()).unwrap()
    }

    /// Unbounded semi-integer decision variable.
    pub fn semi_integer() -> Self {
        Self::new(Kind::SemiInteger, Bound::default(), ATol::default()).unwrap()
    }

    /// Unbounded semi-continuous decision variable.
    pub fn semi_continuous() -> Self {
        Self::new(Kind::SemiContinuous, Bound::default(), ATol::default()).unwrap()
    }

    /// Check if the substituted value is consistent to the bound and kind
    ///
    /// Example
    /// --------
    ///
    /// ```rust
    /// use ommx::{DecisionVariable, Kind, Bound, ATol};
    ///
    /// let dv = DecisionVariable::new(
    ///     Kind::Integer,
    ///     Bound::new(0.0, 2.0).unwrap(),
    ///     ATol::default(),
    /// ).unwrap();
    ///
    /// // 1 \in [0, 2]
    /// assert!(dv.check_value_consistency(0.into(), 1.0, ATol::default()).is_ok());
    /// // 3 \in [0, 2]
    /// assert!(dv.check_value_consistency(0.into(), 3.0, ATol::default()).is_err());
    /// // 0.5 \in [0, 2], but not consistent to Kind::Integer
    /// assert!(dv.check_value_consistency(0.into(), 0.5, ATol::default()).is_err());
    /// ```
    pub fn check_value_consistency(
        &self,
        id: VariableID,
        value: f64,
        atol: ATol,
    ) -> Result<(), DecisionVariableError> {
        let err = || DecisionVariableError::SubstitutedValueInconsistent {
            id,
            kind: self.kind,
            bound: self.bound,
            substituted_value: value,
            atol,
        };
        ensure_finite_value(id, value)?;
        if !self.bound.contains(value, atol) {
            return Err(err());
        }
        match self.kind {
            Kind::Integer | Kind::Binary | Kind::SemiInteger => {
                let rounded = value.round();
                if (rounded - value).abs() >= atol {
                    return Err(err());
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Set a bound on the decision variable by removing the previous bound.
    pub fn set_bound(&mut self, bound: Bound, atol: ATol) -> Result<(), DecisionVariableError> {
        let bound = self.kind.consistent_bound(bound, atol).ok_or(
            DecisionVariableError::BoundInconsistentToKind {
                kind: self.kind,
                bound,
            },
        )?;
        self.bound = bound;
        Ok(())
    }

    /// Call [`Self::set_bound`] in a builder style.
    pub fn with_bound(mut self, bound: Bound, atol: ATol) -> Result<Self, DecisionVariableError> {
        self.set_bound(bound, atol)?;
        Ok(self)
    }

    /// Impose additional bound with current bound by computing their intersection.
    ///
    /// This method computes the intersection of the current bound and the new bound,
    /// then sets the result as the new bound. If the intersection is empty,
    /// an `EmptyBoundIntersection` error is returned.
    ///
    /// Returns `Ok(true)` if the bound was actually changed, `Ok(false)` if the bound
    /// remained the same.
    pub fn clip_bound(
        &mut self,
        id: VariableID,
        bound: Bound,
        atol: ATol,
    ) -> Result<bool, DecisionVariableError> {
        let intersected = self.bound.intersection(&bound).ok_or(
            DecisionVariableError::EmptyBoundIntersection {
                id,
                existing_bound: self.bound,
                new_bound: bound,
            },
        )?;

        // Check if the bound actually changes
        if self.bound.abs_diff_eq(&intersected, atol) {
            Ok(false)
        } else {
            self.set_bound(intersected, atol)?;
            Ok(true)
        }
    }
}

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum DecisionVariableError {
    #[error("Bound is inconsistent to kind: kind={kind:?}, bound={bound}")]
    BoundInconsistentToKind { kind: Kind, bound: Bound },

    #[error("Decision variable value for ID={id} must be finite: value={value}")]
    NonFiniteValue { id: VariableID, value: f64 },

    #[error("Substituted value for ID={id} cannot be overwritten: previous={previous_value}, new={new_value}, atol={atol:?}")]
    SubstitutedValueOverwrite {
        id: VariableID,
        previous_value: f64,
        new_value: f64,
        atol: ATol,
    },

    #[error("Substituted value for ID={id} is inconsistent: kind={kind:?}, bound={bound}, substituted_value={substituted_value}, atol={atol:?}")]
    SubstitutedValueInconsistent {
        id: VariableID,
        kind: Kind,
        bound: Bound,
        substituted_value: f64,
        atol: ATol,
    },

    #[error("Empty bound intersection for ID={id}: existing bound={existing_bound}, new bound={new_bound}")]
    EmptyBoundIntersection {
        id: VariableID,
        existing_bound: Bound,
        new_bound: Bound,
    },
}

/// Modeling label for decision variables.
pub type DecisionVariableLabel = crate::ModelingLabel;

/// Single evaluation result with data integrity guarantees
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct EvaluatedDecisionVariable {
    #[getset(get = "pub")]
    kind: Kind,
    #[getset(get = "pub")]
    bound: Bound,
    #[getset(get = "pub")]
    value: f64,
}

impl EvaluatedDecisionVariable {
    /// Create a new evaluated decision-variable row from an ID, row definition,
    /// and value.
    ///
    /// This method does not enforce kind or bound constraints - those are checked
    /// as part of solution feasibility validation. The ID is used for error
    /// reporting only; the returned row does not store it.
    pub fn new(
        id: VariableID,
        decision_variable: DecisionVariable,
        value: f64,
    ) -> Result<Self, DecisionVariableError> {
        ensure_finite_value(id, value)?;

        // Note: Kind and bound checking is intentionally omitted to allow infeasible solutions.
        // These will be checked as part of Solution::feasible() validation.

        Ok(Self {
            kind: decision_variable.kind,
            bound: decision_variable.bound,
            value,
        })
    }

    /// Check if the value satisfies kind and bound constraints
    pub fn is_valid(&self, atol: crate::ATol) -> bool {
        if !self.value.is_finite() {
            return false;
        }

        // Check bound
        if !self.bound.contains(self.value, atol) {
            return false;
        }

        // Check integrality for integer-like kinds
        match self.kind {
            Kind::Integer | Kind::Binary | Kind::SemiInteger => {
                let rounded = self.value.round();
                (rounded - self.value).abs() < atol
            }
            _ => true,
        }
    }
}

/// Multiple sample evaluation results with deduplication
#[derive(Debug, Clone, Getters)]
pub struct SampledDecisionVariable {
    #[getset(get = "pub")]
    kind: Kind,
    #[getset(get = "pub")]
    bound: Bound,
    #[getset(get = "pub")]
    samples: Sampled<f64>,
}

impl SampledDecisionVariable {
    /// Create a new sampled decision-variable row from an ID, row definition,
    /// and samples.
    ///
    /// This method does not enforce kind or bound constraints - those are checked
    /// as part of sample-set feasibility validation. The ID is used for error
    /// reporting only; the returned row does not store it.
    pub fn new(
        id: VariableID,
        decision_variable: DecisionVariable,
        samples: Sampled<f64>,
    ) -> Result<Self, DecisionVariableError> {
        for (_, &sample_value) in samples.iter() {
            ensure_finite_value(id, sample_value)?;
        }

        // Note: Kind and bound checking is intentionally omitted to allow infeasible solutions.

        Ok(Self {
            kind: decision_variable.kind,
            bound: decision_variable.bound,
            samples,
        })
    }

    /// Get a specific evaluated decision variable by sample ID.
    ///
    /// Returns [`None`] if `sample_id` is not present in the sampled data.
    pub fn get(&self, id: VariableID, sample_id: SampleID) -> Option<EvaluatedDecisionVariable> {
        let value = *self.samples.get(sample_id)?;

        // Create a DecisionVariable to use with EvaluatedDecisionVariable::new
        let dv = DecisionVariable {
            kind: self.kind,
            bound: self.bound,
        };

        Some(EvaluatedDecisionVariable::new(id, dv, value).unwrap())
    }
}

/// Build a v1 `DecisionVariable` from an evaluated variable plus its
/// modeling label. The label comes from the enclosing collection's
/// [`VariableLabelStore`]; the per-element struct no longer carries it.
pub(crate) fn evaluated_decision_variable_to_v1(
    id: VariableID,
    eval_dv: EvaluatedDecisionVariable,
    label: DecisionVariableLabel,
) -> crate::v1::DecisionVariable {
    crate::v1::DecisionVariable {
        id: id.into_inner(),
        kind: eval_dv.kind.into(),
        bound: Some(eval_dv.bound.into()),
        substituted_value: Some(eval_dv.value),
        name: label.name,
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        description: label.description,
    }
}

// NOTE: There are intentionally no `impl From<DecisionVariable> for
// v1::DecisionVariable` (or the Evaluated / Sampled variants). v3 keeps
// modeling labels at the collection layer, so a per-element conversion would
// have to default every label field — silently dropping any caller-supplied
// label. Callers must instead go through
// [`decision_variable::parse::decision_variable_to_v1`] (and the Evaluated
// / Sampled siblings), which take the label explicitly. Top-level
// container serialization (`From<Instance> for v1::Instance` etc.) drains
// the SoA store and threads the label through these helpers.

impl std::convert::TryFrom<crate::v1::DecisionVariable> for EvaluatedDecisionVariable {
    type Error = crate::ParseError;

    fn try_from(v1_dv: crate::v1::DecisionVariable) -> Result<Self, Self::Error> {
        let message = "ommx.v1.DecisionVariable";

        // Parse the DecisionVariable first to get strongly typed fields
        let parsed: parse::ParsedDecisionVariable =
            v1_dv.clone().parse_as(&(), message, "decision_variable")?;
        let dv = parsed.variable;

        // Extract the value from substituted_value (required for EvaluatedDecisionVariable)
        let value = v1_dv.substituted_value.ok_or(
            RawParseError::MissingField {
                message,
                field: "substituted_value",
            }
            .context(message, "substituted_value"),
        )?;

        EvaluatedDecisionVariable::new(parsed.id, dv, value)
            .map_err(|e| crate::RawParseError::InvalidDecisionVariable(e).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1;

    #[test]
    fn test_clip_bound_normal_intersection() {
        // Test case 1: Normal intersection
        let mut dv = DecisionVariable::continuous()
            .with_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
            .unwrap();
        let changed = dv
            .clip_bound(
                VariableID::from(1),
                Bound::new(5.0, 15.0).unwrap(),
                ATol::default(),
            )
            .unwrap();
        assert!(changed);
        assert_eq!(dv.bound(), Bound::new(5.0, 10.0).unwrap());

        // Test case 2: Intersection with infinite bounds
        let mut dv = DecisionVariable::continuous()
            .with_bound(
                Bound::new(f64::NEG_INFINITY, 10.0).unwrap(),
                ATol::default(),
            )
            .unwrap();
        let changed = dv
            .clip_bound(
                VariableID::from(2),
                Bound::new(5.0, f64::INFINITY).unwrap(),
                ATol::default(),
            )
            .unwrap();
        assert!(changed);
        assert_eq!(dv.bound(), Bound::new(5.0, 10.0).unwrap());

        // Test case 3: Clip bound is completely contained
        let mut dv = DecisionVariable::continuous()
            .with_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
            .unwrap();
        let changed = dv
            .clip_bound(
                VariableID::from(3),
                Bound::new(2.0, 8.0).unwrap(),
                ATol::default(),
            )
            .unwrap();
        assert!(changed);
        assert_eq!(dv.bound(), Bound::new(2.0, 8.0).unwrap());

        // Test case 4: No change (clip with same bound)
        let mut dv = DecisionVariable::continuous()
            .with_bound(Bound::new(0.0, 10.0).unwrap(), ATol::default())
            .unwrap();
        let changed = dv
            .clip_bound(
                VariableID::from(4),
                Bound::new(0.0, 10.0).unwrap(),
                ATol::default(),
            )
            .unwrap();
        assert!(!changed);
        assert_eq!(dv.bound(), Bound::new(0.0, 10.0).unwrap());

        // Test case 5: No change (clip with larger bound)
        let changed = dv
            .clip_bound(
                VariableID::from(4),
                Bound::new(-5.0, 15.0).unwrap(),
                ATol::default(),
            )
            .unwrap();
        assert!(!changed);
        assert_eq!(dv.bound(), Bound::new(0.0, 10.0).unwrap());
    }

    #[test]
    fn test_clip_bound_empty_intersection() {
        // Test case 1: Non-overlapping bounds [0, 5] and [10, 15]
        let mut dv = DecisionVariable::continuous()
            .with_bound(Bound::new(0.0, 5.0).unwrap(), ATol::default())
            .unwrap();
        let result = dv.clip_bound(
            VariableID::from(1),
            Bound::new(10.0, 15.0).unwrap(),
            ATol::default(),
        );
        assert!(matches!(
            result,
            Err(DecisionVariableError::EmptyBoundIntersection { .. })
        ));

        // Test case 2: Reverse order
        let mut dv = DecisionVariable::continuous()
            .with_bound(Bound::new(10.0, 15.0).unwrap(), ATol::default())
            .unwrap();
        let result = dv.clip_bound(
            VariableID::from(2),
            Bound::new(0.0, 5.0).unwrap(),
            ATol::default(),
        );
        assert!(matches!(
            result,
            Err(DecisionVariableError::EmptyBoundIntersection { .. })
        ));
    }

    #[test]
    fn test_clip_bound_with_kinds() {
        // Test with Integer kind
        let mut dv = DecisionVariable::integer()
            .with_bound(Bound::new(1.1, 5.9).unwrap(), ATol::default())
            .unwrap();
        assert_eq!(dv.bound(), Bound::new(2.0, 5.0).unwrap()); // Rounded to integer bounds
        let changed = dv
            .clip_bound(
                VariableID::from(1),
                Bound::new(2.1, 4.9).unwrap(),
                ATol::default(),
            )
            .unwrap();
        assert!(changed);
        assert_eq!(dv.bound(), Bound::new(3.0, 4.0).unwrap());

        // Test with Binary kind - clip to [0, 0]
        let mut dv = DecisionVariable::binary();
        assert_eq!(dv.bound(), Bound::new(0.0, 1.0).unwrap());
        let changed = dv
            .clip_bound(
                VariableID::from(2),
                Bound::new(-1.0, 0.5).unwrap(),
                ATol::default(),
            )
            .unwrap();
        assert!(changed);
        assert_eq!(dv.bound(), Bound::new(0.0, 0.0).unwrap());

        // Test with Binary kind - empty intersection
        let mut dv = DecisionVariable::binary();
        let result = dv.clip_bound(
            VariableID::from(3),
            Bound::new(1.1, 2.0).unwrap(),
            ATol::default(),
        );
        assert!(matches!(
            result,
            Err(DecisionVariableError::EmptyBoundIntersection { .. })
        ));
    }

    #[test]
    fn test_decision_variable_rejects_non_finite_values() {
        let id = VariableID::from(1);
        let dv = DecisionVariable::continuous();

        assert!(matches!(
            dv.check_value_consistency(id, f64::NAN, ATol::default()),
            Err(DecisionVariableError::NonFiniteValue { .. })
        ));
        assert!(matches!(
            dv.check_value_consistency(id, f64::INFINITY, ATol::default()),
            Err(DecisionVariableError::NonFiniteValue { .. })
        ));
        assert!(matches!(
            EvaluatedDecisionVariable::new(id, dv, f64::NEG_INFINITY),
            Err(DecisionVariableError::NonFiniteValue { .. })
        ));
    }

    #[test]
    fn test_sampled_decision_variable_rejects_non_finite_values() {
        let id = VariableID::from(1);
        let dv = DecisionVariable::continuous();
        let samples = Sampled::new([vec![crate::SampleID::from(0)]], [f64::NAN]).unwrap();

        assert!(matches!(
            SampledDecisionVariable::new(id, dv, samples),
            Err(DecisionVariableError::NonFiniteValue { .. })
        ));
    }

    #[test]
    fn test_evaluated_decision_variable_try_from() {
        // Test successful conversion
        let v1_dv = v1::DecisionVariable {
            id: 42,
            kind: v1::decision_variable::Kind::Integer as i32,
            bound: Some(v1::Bound {
                lower: 0.0,
                upper: 10.0,
            }),
            substituted_value: Some(5.0),
            name: Some("test_var".to_string()),
            subscripts: vec![1, 2],
            parameters: vec![("param1".to_string(), "value1".to_string())]
                .into_iter()
                .collect(),
            description: Some("A test variable".to_string()),
        };

        let evaluated_dv: EvaluatedDecisionVariable = v1_dv.try_into().unwrap();

        assert_eq!(*evaluated_dv.kind(), crate::Kind::Integer);
        assert_eq!(*evaluated_dv.value(), 5.0);

        // Note: per-element labels are gone in v3; the standalone TryFrom
        // path drops labels. End-to-end name preservation flows through
        // Solution / SampleSet, which carry a VariableLabelStore.
        // Test round-trip conversion at the intrinsic-data level via the
        // explicit `evaluated_decision_variable_to_v1` helper.
        let v1_converted = evaluated_decision_variable_to_v1(
            VariableID::from(42),
            evaluated_dv,
            Default::default(),
        );
        assert_eq!(v1_converted.id, 42);
        assert_eq!(v1_converted.substituted_value, Some(5.0));
    }

    #[test]
    fn test_evaluated_decision_variable_try_from_missing_value() {
        // Test conversion failure when substituted_value is missing
        let v1_dv = v1::DecisionVariable {
            id: 42,
            kind: v1::decision_variable::Kind::Integer as i32,
            bound: Some(v1::Bound {
                lower: 0.0,
                upper: 10.0,
            }),
            substituted_value: None, // Missing value should cause error
            name: Some("test_var".to_string()),
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };

        let result: Result<EvaluatedDecisionVariable, _> = v1_dv.try_into();
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.DecisionVariable[substituted_value]
        Field substituted_value in ommx.v1.DecisionVariable is missing.
        "###);
    }
}
