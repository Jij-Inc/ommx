mod approx;
mod arbitrary;
mod label_store;
mod logical_memory;
pub(crate) mod parse;
mod table;

pub use arbitrary::*;
pub use label_store::VariableLabelStore;
pub use table::*;

use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use crate::{ATol, Bound, Parse, RawParseError, SampleID, Sampled};
use ::approx::AbsDiffEq;
use derive_more::{Deref, From};
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
    LogicalMemoryProfile,
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
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    LogicalMemoryProfile,
)]
pub enum Kind {
    Continuous,
    Integer,
    Binary,
    SemiContinuous,
    SemiInteger,
    /// A decision variable whose domain is an explicitly enumerated finite set.
    FiniteDomain,
}

impl Kind {
    /// Return the exact discrete value represented by `value` under `atol`.
    ///
    /// This is crate-visible so Instance-owned solver-state population can use
    /// the kind's normalization rule without moving state ownership out of
    /// [`crate::Instance`].
    /// Bounds are intentionally not considered here: snapping to the variable's
    /// discrete kind does not make an out-of-bound value feasible, so Solution
    /// feasibility can still report the bound violation.
    pub(crate) fn canonical_discrete_value(&self, value: f64, atol: ATol) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }

        let candidate = match self {
            Kind::Binary => {
                if value < 0.5 {
                    0.0
                } else {
                    1.0
                }
            }
            Kind::Integer | Kind::SemiInteger => value.round(),
            Kind::Continuous | Kind::SemiContinuous | Kind::FiniteDomain => return None,
        };

        if atol.approx_eq(candidate, value) {
            // Normalize negative zero so exact state grouping is deterministic.
            Some(if candidate == 0.0 { 0.0 } else { candidate })
        } else {
            None
        }
    }

    /// Check and convert the bound to a consistent bound
    ///
    /// - For [`Kind::Continuous`] or [`Kind::SemiContinuous`], arbitrary bound is allowed.
    /// - For [`Kind::Integer`] or [`Kind::Binary`], the bound is restricted to integer or binary.
    ///   If there is no integer or binary in the bound, [`None`] is returned.
    /// - For [`Kind::SemiInteger`], the bound is also restricted to integer.
    ///   If there is no integer in the bound, on the other hand, returns `[0.0, 0.0]`.
    /// - [`Kind::FiniteDomain`] cannot be defined by a bound alone and always returns
    ///   [`None`]. Use [`DecisionVariable::new_finite_domain`] instead.
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
                match (bound.contains(0.0, atol), bound.contains(1.0, atol)) {
                    (false, false) => None,
                    (true, false) => Some(Bound::new(0.0, 0.0).unwrap()),
                    (false, true) => Some(Bound::new(1.0, 1.0).unwrap()),
                    (true, true) => Some(Bound::new(0.0, 1.0).unwrap()),
                }
            }
            Kind::FiniteDomain => None,
        }
    }
    // Normalize the supplied f64 endpoints without tolerance.
    // Semi-integer domains retain their implicit zero when no integer remains
    // in the stored interval, just as in consistent_bound.
    fn consistent_bound_exact(&self, bound: Bound) -> Option<Bound> {
        let integer_bound = |bound: Bound| {
            let lower = bound.lower().ceil();
            Bound::new(
                if lower == 0.0 { 0.0 } else { lower },
                bound.upper().floor(),
            )
            .ok()
        };
        match self {
            Kind::Continuous | Kind::SemiContinuous => Some(bound),
            Kind::Integer => integer_bound(bound),
            Kind::Binary => integer_bound(bound.intersection(&Bound::of_binary())?),
            Kind::SemiInteger => {
                Some(integer_bound(bound).unwrap_or_else(|| Bound::new(0.0, 0.0).unwrap()))
            }
            Kind::FiniteDomain => None,
        }
    }
}

/// An explicitly enumerated finite numeric domain.
///
/// A finite domain is the exact feasible set of a decision variable, not an
/// approximation obtained by discretizing a continuous interval. Values are
/// canonicalized into ascending order at construction time.
///
/// # Invariants
///
/// - `values` is non-empty;
/// - every value is finite;
/// - values are strictly increasing and contain no duplicates;
/// - `bound` is the derived convex hull `[min(values), max(values)]`.
///
/// These invariants are enforced by [`FiniteDomain::new`]. The fields are
/// private so SDK callers cannot construct an invalid domain.
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct FiniteDomain {
    values: Vec<f64>,
    bound: Bound,
}

impl FiniteDomain {
    /// Construct a finite domain from explicitly enumerated feasible values.
    pub fn new(mut values: Vec<f64>) -> Result<Self, DecisionVariableError> {
        if values.is_empty() {
            return Err(DecisionVariableError::EmptyFiniteDomain);
        }
        if let Some(&value) = values.iter().find(|value| !value.is_finite()) {
            return Err(DecisionVariableError::NonFiniteDomainValue { value });
        }

        values.sort_by(f64::total_cmp);
        if let Some(values) = values.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(DecisionVariableError::DuplicateFiniteDomainValue { value: values[0] });
        }

        let bound = Bound::new(values[0], values[values.len() - 1]).unwrap();
        Ok(Self { values, bound })
    }

    /// Canonically ordered feasible values.
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Convex hull derived from the first and last feasible values.
    pub fn bound(&self) -> Bound {
        self.bound
    }

    fn contains(&self, value: f64, atol: ATol) -> bool {
        self.values
            .iter()
            .any(|possible| atol.approx_eq(*possible, value))
    }

    fn clipped(&self, bound: Bound) -> Result<Self, DecisionVariableError> {
        let values = self
            .values
            .iter()
            .copied()
            .filter(|value| *value >= bound.lower() && *value <= bound.upper())
            .collect();
        Self::new(values)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Domain {
    Bound(Bound),
    FiniteDomain(FiniteDomain),
}

impl Domain {
    fn contains(&self, value: f64, atol: ATol) -> bool {
        match self {
            Self::Bound(bound) => bound.contains(value, atol),
            Self::FiniteDomain(domain) => domain.contains(value, atol),
        }
    }

    fn bound(&self) -> Bound {
        match self {
            Self::Bound(bound) => *bound,
            Self::FiniteDomain(domain) => domain.bound(),
        }
    }

    fn finite(&self) -> Option<&FiniteDomain> {
        match self {
            Self::Bound(_) => None,
            Self::FiniteDomain(domain) => Some(domain),
        }
    }

    fn bound_ref(&self) -> &Bound {
        match self {
            Self::Bound(bound) => bound,
            Self::FiniteDomain(domain) => &domain.bound,
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
/// Holds its variable `kind` and exact domain as its intrinsic definition. The
/// [`VariableID`] is owned by the enclosing decision-variable table key.
/// Auxiliary modeling label (`name`, `subscripts`, `parameters`,
/// `description`) and fixed values live on the enclosing
/// [`DecisionVariableTable`](crate::DecisionVariableTable) keyed by
/// [`VariableID`].
///
/// Invariants
/// ----------
/// - interval `bound` is normalized for `kind` at construction or bound mutation time.
///   Tolerance-aware operations use the supplied `atol`; exact clipping rounds
///   inward without tolerance.
/// - A [`DecisionVariable`] row therefore never stores an unnormalized
///   integer, binary, or semi-integer bound when built through the safe API.
/// - [`Kind::FiniteDomain`] always owns a validated [`FiniteDomain`]; its reported
///   bound is derived from the domain and is not a second source of truth.
///
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionVariable {
    kind: Kind,
    domain: Domain,
}

// Profile only the active domain representation. Keep the established
// `bound` path for interval-domain variables while exposing finite-domain
// storage under its domain name.
impl LogicalMemoryProfile for DecisionVariable {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.kind
            .visit_logical_memory(path.with("DecisionVariable.kind").as_mut(), visitor);
        match &self.domain {
            Domain::Bound(bound) => {
                bound.visit_logical_memory(path.with("DecisionVariable.bound").as_mut(), visitor)
            }
            Domain::FiniteDomain(domain) => domain.visit_logical_memory(
                path.with("DecisionVariable.finite_domain").as_mut(),
                visitor,
            ),
        }
    }
}

impl DecisionVariable {
    /// Create a new decision variable.
    pub fn new(kind: Kind, bound: Bound, atol: ATol) -> Result<Self, DecisionVariableError> {
        Ok(Self {
            kind,
            domain: Domain::Bound(
                kind.consistent_bound(bound, atol)
                    .ok_or(DecisionVariableError::BoundInconsistentToKind { kind, bound })?,
            ),
        })
    }

    /// Create a finite-domain decision variable.
    pub fn new_finite_domain(values: Vec<f64>) -> Result<Self, DecisionVariableError> {
        Ok(Self {
            kind: Kind::FiniteDomain,
            domain: Domain::FiniteDomain(FiniteDomain::new(values)?),
        })
    }

    /// Kind of this decision variable.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Convex-hull bound of this decision variable's exact domain.
    pub fn bound(&self) -> Bound {
        self.domain.bound()
    }

    fn bound_ref(&self) -> &Bound {
        self.domain.bound_ref()
    }

    /// Return the exact finite domain, or [`None`] for interval-domain kinds.
    pub fn finite_domain(&self) -> Option<&FiniteDomain> {
        self.domain.finite()
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
            bound: self.bound(),
            substituted_value: value,
            atol,
        };
        ensure_finite_value(id, value)?;
        if !self.domain.contains(value, atol) {
            return Err(err());
        }
        match self.kind {
            Kind::Integer | Kind::Binary | Kind::SemiInteger => {
                let rounded = value.round();
                if !atol.approx_eq(rounded, value) {
                    return Err(err());
                }
            }
            Kind::Continuous | Kind::SemiContinuous | Kind::FiniteDomain => {}
        }
        Ok(())
    }

    /// Set a bound on the decision variable by replacing the previous bound.
    ///
    /// For a finite-domain variable, this retains only the enumerated values
    /// contained in `bound` and derives the new bound from those values.
    pub fn set_bound(&mut self, bound: Bound, atol: ATol) -> Result<(), DecisionVariableError> {
        self.domain = match &self.domain {
            Domain::Bound(_) => Domain::Bound(self.kind.consistent_bound(bound, atol).ok_or(
                DecisionVariableError::BoundInconsistentToKind {
                    kind: self.kind,
                    bound,
                },
            )?),
            Domain::FiniteDomain(domain) => Domain::FiniteDomain(domain.clipped(bound)?),
        };
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
        if let Domain::FiniteDomain(domain) = &self.domain {
            let clipped = domain.clipped(bound).map_err(|_| {
                DecisionVariableError::EmptyBoundIntersection {
                    id,
                    existing_bound: domain.bound(),
                    new_bound: bound,
                }
            })?;
            if &clipped == domain {
                return Ok(false);
            }
            self.domain = Domain::FiniteDomain(clipped);
            return Ok(true);
        }

        let existing_bound = self.bound();
        let intersected = existing_bound.intersection(&bound).ok_or(
            DecisionVariableError::EmptyBoundIntersection {
                id,
                existing_bound,
                new_bound: bound,
            },
        )?;

        // Check if the bound actually changes
        if existing_bound.abs_diff_eq(&intersected, atol) {
            Ok(false)
        } else {
            self.set_bound(intersected, atol)?;
            Ok(true)
        }
    }

    /// Intersect the stored bound with `bound` without tolerance.
    ///
    /// The supplied f64 endpoints are treated as exact boundaries. Continuous
    /// bounds retain even the smallest representable change. Integer bounds
    /// use `ceil(lower)` and `floor(upper)`; binary bounds retain only 0 and 1
    /// inside the intersection. Semi-integer bounds fall back to `[0, 0]` when
    /// no integer remains in their intersected stored interval.
    /// Finite domains retain only their enumerated values inside `bound`.
    ///
    /// Returns whether the normalized stored bound changed. An empty interval
    /// intersection or an interval incompatible with the kind leaves the
    /// variable unchanged and returns an error. This method does not compensate
    /// for rounding errors introduced when computing the supplied endpoints.
    pub fn clip_bound_exact(
        &mut self,
        id: VariableID,
        bound: Bound,
    ) -> Result<bool, DecisionVariableError> {
        if self.finite_domain().is_some() {
            // Finite-domain clipping always intersects the enumerated values exactly.
            return self.clip_bound(id, bound, ATol::default());
        }
        let existing_bound = self.bound();
        let intersected = existing_bound.intersection(&bound).ok_or(
            DecisionVariableError::EmptyBoundIntersection {
                id,
                existing_bound,
                new_bound: bound,
            },
        )?;
        let normalized = self.kind.consistent_bound_exact(intersected).ok_or(
            DecisionVariableError::BoundInconsistentToKind {
                kind: self.kind,
                bound: intersected,
            },
        )?;
        let changed = existing_bound != normalized;
        self.domain = Domain::Bound(normalized);
        Ok(changed)
    }

    // Used by the child table module when exact clipping checks fixed values.
    fn check_value_consistency_exact(&self, id: VariableID, value: f64) -> crate::Result<()> {
        ensure_finite_value(id, value)?;
        let integral = !matches!(self.kind, Kind::Integer | Kind::Binary | Kind::SemiInteger)
            || value == value.round();
        let bound = self.bound();
        let member = self
            .finite_domain()
            .is_none_or(|domain| domain.values().contains(&value));
        if value < bound.lower() || value > bound.upper() || !integral || !member {
            crate::bail!({ ?id, kind = ?self.kind, ?bound, value },
                "Fixed value {value} for variable {id:?} is inconsistent with its exact bound or kind");
        }
        Ok(())
    }
}

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum DecisionVariableError {
    #[error("Bound is inconsistent to kind: kind={kind:?}, bound={bound}")]
    BoundInconsistentToKind { kind: Kind, bound: Bound },

    #[error("Finite decision-variable domain must contain at least one value")]
    EmptyFiniteDomain,

    #[error("Finite decision-variable domain values must be finite: value={value}")]
    NonFiniteDomainValue { value: f64 },

    #[error("Finite decision-variable domain values must be unique: value={value}")]
    DuplicateFiniteDomainValue { value: f64 },

    #[error("Finite domain is only valid for Kind::FiniteDomain, but kind is {kind:?}")]
    UnexpectedFiniteDomain { kind: Kind },

    #[error("Invalid decision variable ID={id}: {source}")]
    InvalidDefinition {
        id: VariableID,
        source: Box<DecisionVariableError>,
    },

    #[error("Duplicate decision variable ID={id}")]
    DuplicateID { id: VariableID },

    #[error(
        "Insufficient capacity for automatically assigned decision variable IDs above the current maximum"
    )]
    NoAvailableID,

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
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedDecisionVariable {
    decision_variable: DecisionVariable,
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
            decision_variable,
            value,
        })
    }

    /// Kind of the evaluated variable.
    pub fn kind(&self) -> &Kind {
        &self.decision_variable.kind
    }

    /// Convex-hull bound of the evaluated variable's exact domain.
    pub fn bound(&self) -> &Bound {
        self.decision_variable.bound_ref()
    }

    /// Exact finite domain, if this is a finite-domain variable.
    pub fn finite_domain(&self) -> Option<&FiniteDomain> {
        self.decision_variable.finite_domain()
    }

    /// Evaluated value.
    pub fn value(&self) -> &f64 {
        &self.value
    }

    /// Check if the value satisfies kind and bound constraints
    pub fn is_valid(&self, atol: crate::ATol) -> bool {
        self.decision_variable
            .check_value_consistency(VariableID::from(0), self.value, atol)
            .is_ok()
    }
}

/// Multiple sample evaluation results with deduplication
#[derive(Debug, Clone)]
pub struct SampledDecisionVariable {
    decision_variable: DecisionVariable,
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
            decision_variable,
            samples,
        })
    }

    /// Kind of the sampled variable.
    pub fn kind(&self) -> &Kind {
        &self.decision_variable.kind
    }

    /// Convex-hull bound of the sampled variable's exact domain.
    pub fn bound(&self) -> &Bound {
        self.decision_variable.bound_ref()
    }

    /// Exact finite domain, if this is a finite-domain variable.
    pub fn finite_domain(&self) -> Option<&FiniteDomain> {
        self.decision_variable.finite_domain()
    }

    /// Sampled values.
    pub fn samples(&self) -> &Sampled<f64> {
        &self.samples
    }

    /// Borrowed validity query for `SampleSetBuilder` to derive feasibility
    /// without materializing an evaluated row. Missing samples are invalid.
    pub(crate) fn is_valid_for(&self, id: VariableID, sample_id: SampleID, atol: ATol) -> bool {
        self.samples.get(sample_id).is_some_and(|&value| {
            self.decision_variable
                .check_value_consistency(id, value, atol)
                .is_ok()
        })
    }

    /// Get a specific evaluated decision variable by sample ID.
    ///
    /// Returns [`None`] if `sample_id` is not present in the sampled data.
    pub fn get(&self, id: VariableID, sample_id: SampleID) -> Option<EvaluatedDecisionVariable> {
        let value = *self.samples.get(sample_id)?;

        Some(EvaluatedDecisionVariable::new(id, self.decision_variable.clone(), value).unwrap())
    }
}

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

        EvaluatedDecisionVariable::new(parsed.id, dv, value).map_err(crate::ParseError::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1;

    #[test]
    fn exact_clipping_retains_sub_tolerance_continuous_updates() {
        let mut exact = DecisionVariable::continuous()
            .with_bound(Bound::new(0.0, 3.0).unwrap(), ATol::default())
            .unwrap();
        let mut tolerant = exact.clone();
        let candidate = Bound::new(f64::from_bits(1), 3.0_f64.next_down()).unwrap();
        assert!(!tolerant
            .clip_bound(0.into(), candidate, ATol::new(1e-6).unwrap())
            .unwrap());
        assert!(exact.clip_bound_exact(0.into(), candidate).unwrap());
        assert_eq!(exact.bound(), candidate);
        assert!(!exact.clip_bound_exact(0.into(), candidate).unwrap());
    }

    #[test]
    fn exact_clipping_rounds_discrete_bounds_inward() {
        for (kind, candidate, expected) in [
            (
                Kind::Integer,
                Bound::new(-3.0_f64.next_down(), 3.0_f64.next_down()).unwrap(),
                Bound::new(-2.0, 2.0).unwrap(),
            ),
            (
                Kind::Binary,
                Bound::new(0.0, 1.0_f64.next_down()).unwrap(),
                Bound::new(0.0, 0.0).unwrap(),
            ),
            (
                Kind::Binary,
                Bound::new(f64::from_bits(1), 1.0).unwrap(),
                Bound::new(1.0, 1.0).unwrap(),
            ),
            (
                Kind::SemiInteger,
                Bound::new(1.25, 1.75).unwrap(),
                Bound::new(0.0, 0.0).unwrap(),
            ),
        ] {
            let mut variable =
                DecisionVariable::new(kind, Bound::unbounded(), ATol::default()).unwrap();
            assert!(variable.clip_bound_exact(0.into(), candidate).unwrap());
            assert_eq!(variable.bound(), expected);
            assert!(!variable
                .clip_bound_exact(0.into(), Bound::unbounded())
                .unwrap());
        }
        for kind in [
            Kind::Integer,
            Kind::Continuous,
            Kind::SemiInteger,
            Kind::SemiContinuous,
        ] {
            let mut variable =
                DecisionVariable::new(kind, Bound::unbounded(), ATol::default()).unwrap();
            assert!(!variable
                .clip_bound_exact(0.into(), Bound::unbounded())
                .unwrap());
        }
    }

    #[test]
    fn exact_clipping_errors_are_atomic_and_normalized_noops_are_reported() {
        for (kind, candidate) in [
            (Kind::Continuous, Bound::new(4.0, 5.0).unwrap()),
            (Kind::Integer, Bound::new(1.25, 1.75).unwrap()),
            (Kind::Binary, Bound::new(0.25, 0.75).unwrap()),
        ] {
            let mut variable =
                DecisionVariable::new(kind, Bound::new(0.0, 3.0).unwrap(), ATol::default())
                    .unwrap();
            let before = variable.clone();
            assert!(variable.clip_bound_exact(0.into(), candidate).is_err());
            assert_eq!(variable, before);
        }
        let mut variable = DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        variable
            .clip_bound_exact(0.into(), Bound::new(0.1, 2.9).unwrap())
            .unwrap();
        assert!(!variable
            .clip_bound_exact(0.into(), Bound::new(0.1, 2.9).unwrap())
            .unwrap());
        assert_eq!(variable.bound(), Bound::new(1.0, 2.0).unwrap());
    }

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
    fn integer_and_binary_bounds_use_bound_membership() {
        let atol = ATol::new(1.0e-6).unwrap();

        let integer_source = Bound::new(2.0 + atol.into_inner(), 4.0 - atol.into_inner()).unwrap();
        assert_eq!(
            Kind::Integer.consistent_bound(integer_source, atol),
            Some(Bound::new(3.0, 3.0).unwrap())
        );
        assert_eq!(
            Kind::SemiInteger.consistent_bound(integer_source, atol),
            Some(Bound::new(3.0, 3.0).unwrap())
        );

        let excludes_one = Bound::new(0.0, 1.0 - atol.into_inner()).unwrap();
        assert!(excludes_one.contains(0.0, atol));
        assert!(!excludes_one.contains(1.0, atol));
        assert_eq!(
            Kind::Binary.consistent_bound(excludes_one, atol),
            Some(Bound::new(0.0, 0.0).unwrap())
        );

        let excludes_zero = Bound::new(atol.into_inner().next_up(), 1.0).unwrap();
        assert!(!excludes_zero.contains(0.0, atol));
        assert!(excludes_zero.contains(1.0, atol));
        assert_eq!(
            Kind::Binary.consistent_bound(excludes_zero, atol),
            Some(Bound::new(1.0, 1.0).unwrap())
        );

        let excludes_both = Bound::new(0.1, 0.9).unwrap();
        assert!(!excludes_both.contains(0.0, atol));
        assert!(!excludes_both.contains(1.0, atol));
        assert_eq!(Kind::Binary.consistent_bound(excludes_both, atol), None);

        assert_eq!(
            Kind::Binary.consistent_bound(Bound::of_binary(), atol),
            Some(Bound::of_binary())
        );
    }

    #[test]
    fn finite_domain_is_canonical_and_exact() {
        let id = VariableID::from(1);
        let variable = DecisionVariable::new_finite_domain(vec![1.0, 0.1, 0.5, 0.3]).unwrap();

        assert_eq!(variable.kind(), Kind::FiniteDomain);
        assert_eq!(
            variable.finite_domain().unwrap().values(),
            &[0.1, 0.3, 0.5, 1.0]
        );
        assert_eq!(variable.bound(), Bound::new(0.1, 1.0).unwrap());
        assert!(variable
            .check_value_consistency(id, 0.3, ATol::default())
            .is_ok());
        assert!(variable
            .check_value_consistency(id, 0.4, ATol::default())
            .is_err());
        assert!(variable
            .check_value_consistency(id, 0.3 + *ATol::default(), ATol::default())
            .is_ok());
    }

    #[test]
    fn finite_domain_membership_does_not_depend_on_hull_residuals() {
        let id = VariableID::from(1);
        let variable = DecisionVariable::new_finite_domain(vec![-f64::MAX, f64::MAX]).unwrap();
        for value in [-f64::MAX, f64::MAX] {
            variable
                .check_value_consistency(id, value, ATol::default())
                .unwrap();
        }
        assert!(matches!(
            variable.check_value_consistency(id, 0.0, ATol::default()),
            Err(DecisionVariableError::SubstitutedValueInconsistent { .. })
        ));
        for value in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
            assert!(matches!(
                variable.check_value_consistency(id, value, ATol::default()),
                Err(DecisionVariableError::NonFiniteValue { .. })
            ));
        }
    }

    #[test]
    fn finite_domain_membership_uses_finite_inclusive_tolerance() {
        let id = VariableID::from(1);
        let variable = DecisionVariable::new_finite_domain(vec![1.0, 3.0]).unwrap();
        let atol = ATol::new(0.25).unwrap();
        for value in [0.75, 1.25, 2.75, 3.25] {
            variable.check_value_consistency(id, value, atol).unwrap();
        }
        for value in [
            0.75_f64.next_down(),
            1.25_f64.next_up(),
            2.75_f64.next_down(),
            3.25_f64.next_up(),
        ] {
            assert!(variable.check_value_consistency(id, value, atol).is_err());
        }

        let domain = FiniteDomain::new(vec![-f64::MAX]).unwrap();
        assert!(!domain.contains(f64::MAX, ATol::new(f64::INFINITY).unwrap()));
    }

    #[test]
    fn finite_domain_rejects_invalid_definitions() {
        assert!(matches!(
            DecisionVariable::new_finite_domain(vec![]),
            Err(DecisionVariableError::EmptyFiniteDomain)
        ));
        assert!(matches!(
            DecisionVariable::new_finite_domain(vec![0.0, f64::NAN]),
            Err(DecisionVariableError::NonFiniteDomainValue { .. })
        ));
        assert!(matches!(
            DecisionVariable::new_finite_domain(vec![0.0, 0.0]),
            Err(DecisionVariableError::DuplicateFiniteDomainValue { .. })
        ));
    }

    #[test]
    fn clipping_finite_domain_filters_values() {
        let id = VariableID::from(1);
        let mut variable = DecisionVariable::new_finite_domain(vec![0.1, 0.3, 0.5, 1.0]).unwrap();

        assert!(variable
            .clip_bound(id, Bound::new(0.2, 0.6).unwrap(), ATol::default())
            .unwrap());
        assert_eq!(variable.finite_domain().unwrap().values(), &[0.3, 0.5]);
        assert_eq!(variable.bound(), Bound::new(0.3, 0.5).unwrap());

        assert!(matches!(
            variable.clip_bound(id, Bound::new(0.31, 0.49).unwrap(), ATol::default()),
            Err(DecisionVariableError::EmptyBoundIntersection { .. })
        ));
    }

    #[test]
    fn exact_clipping_preserves_finite_domain_and_is_atomic() {
        let id = VariableID::from(1);
        let mut variable = DecisionVariable::new_finite_domain(vec![0.1, 0.3, 0.5, 1.0]).unwrap();
        assert!(variable
            .clip_bound_exact(id, Bound::new(0.1_f64.next_up(), 0.5).unwrap())
            .unwrap());
        assert_eq!(variable.finite_domain().unwrap().values(), &[0.3, 0.5]);
        let original = variable.clone();
        assert!(variable
            .clip_bound_exact(id, Bound::new(0.31, 0.49).unwrap())
            .is_err());
        assert_eq!(variable, original);
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
    fn canonical_discrete_value_includes_the_atol_boundary() {
        let atol = ATol::new(0.125).unwrap();

        assert_eq!(
            Kind::Binary.canonical_discrete_value(-0.0625, atol),
            Some(0.0)
        );
        assert_eq!(
            Kind::Binary.canonical_discrete_value(1.0625, atol),
            Some(1.0)
        );
        assert_eq!(
            Kind::Integer.canonical_discrete_value(-2.0625, atol),
            Some(-2.0)
        );
        assert_eq!(
            Kind::SemiInteger.canonical_discrete_value(-1.9375, atol),
            Some(-2.0)
        );

        assert_eq!(
            Kind::Binary.canonical_discrete_value(1.125, atol),
            Some(1.0)
        );
        assert_eq!(
            Kind::Integer.canonical_discrete_value(-2.125, atol),
            Some(-2.0)
        );
        assert_eq!(
            Kind::SemiInteger.canonical_discrete_value(-1.875, atol),
            Some(-2.0)
        );

        assert_eq!(
            Kind::Binary.canonical_discrete_value(1.1250000000000002, atol),
            None
        );
        assert_eq!(
            Kind::Integer.canonical_discrete_value(-2.1250000000000004, atol),
            None
        );
        assert_eq!(
            Kind::SemiInteger.canonical_discrete_value(-1.8749999999999998, atol),
            None
        );
        assert_eq!(Kind::Integer.canonical_discrete_value(-2.25, atol), None);
        assert_eq!(Kind::Continuous.canonical_discrete_value(1.125, atol), None);
        assert_eq!(
            Kind::SemiContinuous.canonical_discrete_value(1.125, atol),
            None
        );

        let zero_atol = ATol::new(0.5).unwrap();
        for kind in [Kind::Integer, Kind::SemiInteger] {
            let zero = kind.canonical_discrete_value(-0.25, zero_atol).unwrap();
            assert_eq!(zero.to_bits(), 0.0_f64.to_bits());
        }

        let large_atol = ATol::new(2.0).unwrap();
        assert_eq!(
            Kind::Binary.canonical_discrete_value(1.5, large_atol),
            Some(1.0)
        );
    }

    #[test]
    fn test_sampled_decision_variable_rejects_non_finite_values() {
        let id = VariableID::from(1);
        for dv in [
            DecisionVariable::continuous(),
            DecisionVariable::new_finite_domain(vec![1.0, 3.0]).unwrap(),
        ] {
            for value in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
                let samples = Sampled::from((SampleID::from(0), value));
                assert!(matches!(
                    SampledDecisionVariable::new(id, dv.clone(), samples),
                    Err(DecisionVariableError::NonFiniteValue { .. })
                ));
            }
        }
    }

    #[test]
    fn borrowed_sample_validity_matches_evaluated_rows() {
        let id = VariableID::from(1);
        let sample_id = SampleID::from(4);
        let missing_id = SampleID::from(5);
        for variable in [
            DecisionVariable::binary(),
            DecisionVariable::integer(),
            DecisionVariable::continuous(),
            DecisionVariable::semi_integer(),
            DecisionVariable::semi_continuous(),
            DecisionVariable::new_finite_domain(vec![1.0, 3.0]).unwrap(),
            DecisionVariable::new_finite_domain(vec![-f64::MAX, f64::MAX]).unwrap(),
        ] {
            for value in [
                -f64::MAX,
                -1.0,
                0.0,
                0.75,
                1.0,
                1.25,
                1.25_f64.next_up(),
                2.0,
                3.0,
                f64::MAX,
            ] {
                let sampled = SampledDecisionVariable::new(
                    id,
                    variable.clone(),
                    Sampled::from((sample_id, value)),
                )
                .unwrap();
                for tolerance in [f64::EPSILON, 0.25, f64::INFINITY] {
                    let atol = ATol::new(tolerance).unwrap();
                    assert_eq!(
                        sampled.is_valid_for(id, sample_id, atol),
                        sampled.get(id, sample_id).unwrap().is_valid(atol),
                        "variable={variable:?}, value={value}, atol={atol:?}"
                    );
                    assert!(!sampled.is_valid_for(id, missing_id, atol));
                }
            }
        }
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
            finite_domain: None,
        };

        let evaluated_dv: EvaluatedDecisionVariable = v1_dv.try_into().unwrap();

        assert_eq!(*evaluated_dv.kind(), crate::Kind::Integer);
        assert_eq!(*evaluated_dv.value(), 5.0);

        // Note: per-element labels are gone in v3; the standalone TryFrom
        // path drops labels. End-to-end name preservation flows through
        // Solution / SampleSet, which carry a VariableLabelStore.
        // Test round-trip conversion at the table level, where labels live.
        let table = EvaluatedDecisionVariableTable::new(
            std::collections::BTreeMap::from([(VariableID::from(42), evaluated_dv)]),
            VariableLabelStore::default(),
        )
        .unwrap();
        let mut rows: Vec<v1::DecisionVariable> = (&table).into();
        let v1_converted = rows.pop().unwrap();
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
            finite_domain: None,
        };

        let result: Result<EvaluatedDecisionVariable, _> = v1_dv.try_into();
        insta::assert_snapshot!(result.unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.DecisionVariable[substituted_value]
        Field substituted_value in ommx.v1.DecisionVariable is missing.
        "###);
    }
}
