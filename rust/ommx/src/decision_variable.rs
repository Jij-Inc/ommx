mod arbitrary;
mod parse;

pub use arbitrary::*;
use getset::CopyGetters;

use crate::{sampled::UnknownSampleIDError, ATol, Bound, SampleID, Sampled};
use derive_more::{Deref, From};
use fnv::FnvHashMap;
use getset::Getters;
use std::collections::BTreeSet;

/// ID for decision variable and parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Deref)]
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

impl std::fmt::Display for VariableID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
                let bound = bound.as_integer_bound(atol)?;
                if bound.contains(0.0, atol) || bound.contains(1.0, atol) {
                    Some(bound)
                } else {
                    None
                }
            }
        }
    }
}

/// The decision variable with metadata.
///
/// Invariants
/// ----------
/// - `kind` and `bound` are consistent
///   - i.e. `bound` is invariant under `|bound| kind.consistent_bound(bound, atol).unwrap()` for appropriate `atol`.
/// - If `substituted_value` is set, it is consistent to `kind` and `bound`.
///
#[derive(Debug, Clone, PartialEq, CopyGetters)]
pub struct DecisionVariable {
    #[getset(get_copy = "pub")]
    id: VariableID,
    #[getset(get_copy = "pub")]
    kind: Kind,
    #[getset(get_copy = "pub")]
    bound: Bound,
    #[getset(get_copy = "pub")]
    substituted_value: Option<f64>,

    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

impl DecisionVariable {
    /// Create a new decision variable.
    pub fn new(
        id: VariableID,
        kind: Kind,
        bound: Bound,
        substituted_value: Option<f64>,
        atol: ATol,
    ) -> Result<Self, DecisionVariableError> {
        let mut new = Self {
            id,
            kind,
            bound: kind
                .consistent_bound(bound, atol)
                .ok_or(DecisionVariableError::BoundInconsistentToKind { id, kind, bound })?,
            substituted_value: None, // will be set later
            name: None,
            subscripts: Vec::new(),
            parameters: FnvHashMap::default(),
            description: None,
        };
        if let Some(substituted_value) = substituted_value {
            new.check_value_consistency(substituted_value, atol)?;
            new.substituted_value = Some(substituted_value);
        }
        Ok(new)
    }

    pub fn binary(id: VariableID) -> Self {
        Self::new(id, Kind::Binary, Bound::of_binary(), None, ATol::default()).unwrap()
    }

    /// Unbounded integer decision variable.
    pub fn integer(id: VariableID) -> Self {
        Self::new(id, Kind::Integer, Bound::default(), None, ATol::default()).unwrap()
    }

    /// Unbounded continuous decision variable.
    pub fn continuous(id: VariableID) -> Self {
        Self::new(
            id,
            Kind::Continuous,
            Bound::default(),
            None,
            ATol::default(),
        )
        .unwrap()
    }

    /// Unbounded semi-integer decision variable.
    pub fn semi_integer(id: VariableID) -> Self {
        // substituted_value is None, so it is always valid
        Self::new(
            id,
            Kind::SemiInteger,
            Bound::default(),
            None,
            ATol::default(),
        )
        .unwrap()
    }

    /// Unbounded semi-continuous decision variable.
    pub fn semi_continuous(id: VariableID) -> Self {
        // substituted_value is None, so it is always valid
        Self::new(
            id,
            Kind::SemiContinuous,
            Bound::default(),
            None,
            ATol::default(),
        )
        .unwrap()
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
    ///     0.into(),
    ///     Kind::Integer,
    ///     Bound::new(0.0, 2.0).unwrap(),
    ///     None,
    ///     ATol::default(),
    /// ).unwrap();
    ///
    /// // 1 \in [0, 2]
    /// assert!(dv.check_value_consistency(1.0, ATol::default()).is_ok());
    /// // 3 \in [0, 2]
    /// assert!(dv.check_value_consistency(3.0, ATol::default()).is_err());
    /// // 0.5 \in [0, 2], but not consistent to Kind::Integer
    /// assert!(dv.check_value_consistency(0.5, ATol::default()).is_err());
    /// ```
    pub fn check_value_consistency(
        &self,
        value: f64,
        atol: ATol,
    ) -> Result<(), DecisionVariableError> {
        let err = || DecisionVariableError::SubstitutedValueInconsistent {
            id: self.id,
            kind: self.kind,
            bound: self.bound,
            substituted_value: value,
            atol,
        };
        if !self.bound.contains(value, atol) {
            return Err(err());
        }
        match self.kind {
            Kind::Integer | Kind::Binary | Kind::SemiInteger => {
                if value.fract().abs() >= atol {
                    return Err(err());
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn set_bound(&mut self, bound: Bound, atol: ATol) -> Result<(), DecisionVariableError> {
        let bound = self.kind.consistent_bound(bound, atol).ok_or(
            DecisionVariableError::BoundInconsistentToKind {
                id: self.id,
                kind: self.kind,
                bound,
            },
        )?;
        self.bound = bound;
        Ok(())
    }

    pub fn substitute(&mut self, new_value: f64, atol: ATol) -> Result<(), DecisionVariableError> {
        if let Some(previous_value) = self.substituted_value {
            if (new_value - previous_value).abs() > atol {
                return Err(DecisionVariableError::SubstitutedValueOverwrite {
                    id: self.id,
                    previous_value,
                    new_value,
                });
            }
        } else {
            self.check_value_consistency(new_value, atol)?;
            self.substituted_value = Some(new_value);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecisionVariableError {
    #[error("Bound for ID={id} is inconsistent to kind: kind={kind:?}, bound={bound}")]
    BoundInconsistentToKind {
        id: VariableID,
        kind: Kind,
        bound: Bound,
    },

    #[error("Substituted value for ID={id} cannot be overwrite: previous={previous_value}, new={new_value}")]
    SubstitutedValueOverwrite {
        id: VariableID,
        previous_value: f64,
        new_value: f64,
    },

    #[error("Substituted value for ID={id} is inconsistent: kind={kind:?}, bound={bound}, substituted_value={substituted_value}, atol={atol:?}")]
    SubstitutedValueInconsistent {
        id: VariableID,
        kind: Kind,
        bound: Bound,
        substituted_value: f64,
        atol: ATol,
    },
}

/// Auxiliary metadata for decision variables (excluding essential id, kind, bound, substituted_value)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DecisionVariableMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

/// Single evaluation result with data integrity guarantees
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct EvaluatedDecisionVariable {
    #[getset(get = "pub")]
    id: VariableID,
    #[getset(get = "pub")]
    kind: Kind,
    #[getset(get = "pub")]
    bound: Bound,
    #[getset(get = "pub")]
    value: f64,
    pub metadata: DecisionVariableMetadata,
}

/// Multiple sample evaluation results with deduplication
#[derive(Debug, Clone, Getters)]
pub struct SampledDecisionVariable {
    #[getset(get = "pub")]
    id: VariableID,
    #[getset(get = "pub")]
    kind: Kind,
    #[getset(get = "pub")]
    bound: Bound,
    pub metadata: DecisionVariableMetadata,
    #[getset(get = "pub")]
    samples: Sampled<f64>,
}

impl EvaluatedDecisionVariable {
    /// Create a new EvaluatedDecisionVariable (internal use only)
    pub(crate) fn new_internal(
        id: VariableID,
        kind: Kind,
        bound: Bound,
        value: f64,
        metadata: DecisionVariableMetadata,
    ) -> Self {
        Self {
            id,
            kind,
            bound,
            value,
            metadata,
        }
    }

    /// Convert to DecisionVariable with the evaluated value as substituted_value
    pub fn to_decision_variable(&self) -> Result<DecisionVariable, DecisionVariableError> {
        let mut dv = DecisionVariable {
            id: self.id,
            kind: self.kind,
            bound: self.bound,
            substituted_value: None,
            name: self.metadata.name.clone(),
            subscripts: self.metadata.subscripts.clone(),
            parameters: self.metadata.parameters.clone(),
            description: self.metadata.description.clone(),
        };

        dv.check_value_consistency(self.value, ATol::default())?;
        dv.substituted_value = Some(self.value);

        Ok(dv)
    }
}

impl SampledDecisionVariable {
    /// Create a new SampledDecisionVariable (internal use only)
    pub(crate) fn new_internal(
        id: VariableID,
        kind: Kind,
        bound: Bound,
        metadata: DecisionVariableMetadata,
        samples: Sampled<f64>,
    ) -> Self {
        Self {
            id,
            kind,
            bound,
            metadata,
            samples,
        }
    }

    /// Get a specific evaluated decision variable by sample ID
    pub fn get(
        &self,
        sample_id: SampleID,
    ) -> Result<EvaluatedDecisionVariable, UnknownSampleIDError> {
        let value = *self.samples.get(sample_id)?;

        Ok(EvaluatedDecisionVariable::new_internal(
            self.id,
            self.kind,
            self.bound,
            value,
            self.metadata.clone(),
        ))
    }
}

impl crate::Evaluate for DecisionVariable {
    type Output = EvaluatedDecisionVariable;
    type SampledOutput = SampledDecisionVariable;

    fn evaluate(
        &self,
        state: &crate::v1::State,
        _atol: crate::ATol,
    ) -> anyhow::Result<Self::Output> {
        let value = state
            .entries
            .get(&self.id.into_inner())
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Variable ID {} not found in state", self.id))?;

        Ok(EvaluatedDecisionVariable::new_internal(
            self.id,
            self.kind,
            self.bound,
            value,
            DecisionVariableMetadata {
                name: self.name.clone(),
                subscripts: self.subscripts.clone(),
                parameters: self.parameters.clone(),
                description: self.description.clone(),
            },
        ))
    }

    fn evaluate_samples(
        &self,
        samples: &crate::v1::Samples,
        _atol: crate::ATol,
    ) -> anyhow::Result<Self::SampledOutput> {
        let variable_id = self.id.into_inner();

        // Extract values for this variable from all samples
        let mut grouped_values: std::collections::HashMap<
            ordered_float::OrderedFloat<f64>,
            Vec<crate::SampleID>,
        > = std::collections::HashMap::new();
        for (sample_id, state) in samples.iter() {
            if let Some(value) = state.entries.get(&variable_id) {
                grouped_values
                    .entry(ordered_float::OrderedFloat(*value))
                    .or_default()
                    .push(crate::SampleID::from(*sample_id));
            }
        }

        // Convert to Sampled format
        let ids: Vec<Vec<crate::SampleID>> = grouped_values.values().cloned().collect();
        let values: Vec<f64> = grouped_values.keys().map(|k| k.into_inner()).collect();
        let samples = crate::Sampled::new(ids, values)?;

        Ok(SampledDecisionVariable::new_internal(
            self.id,
            self.kind,
            self.bound,
            DecisionVariableMetadata {
                name: self.name.clone(),
                subscripts: self.subscripts.clone(),
                parameters: self.parameters.clone(),
                description: self.description.clone(),
            },
            samples,
        ))
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> anyhow::Result<()> {
        if let Some(value) = state.entries.get(&self.id.into_inner()) {
            self.substitute(*value, atol)?;
        }
        Ok(())
    }

    fn required_ids(&self) -> crate::VariableIDSet {
        [self.id].into_iter().collect()
    }
}
