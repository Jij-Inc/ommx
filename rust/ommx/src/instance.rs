mod analysis;
mod approx;
pub(crate) mod arbitrary;
mod builder;
mod clip_bounds;
mod constraint_hints;
mod convert;
mod decision_variable;
mod error;
mod evaluate;
mod log_encode;
mod logical_memory;
mod named_function;
mod new;
mod parametric_builder;
mod parse;
mod pass;
mod penalty;
mod reduce_binary_power;
mod serialize;
mod setter;
mod stats;
mod substitute;

pub use analysis::*;
pub use builder::*;
pub use error::*;
pub use log_encode::*;
pub use parametric_builder::*;
pub use stats::*;

use crate::{
    constraint_hints::ConstraintHints,
    constraint_type::ConstraintCollection,
    indicator_constraint::{IndicatorConstraint, RemovedIndicatorConstraint},
    named_function::NamedFunctionID,
    parse::Parse,
    v1, AcyclicAssignments, Constraint, ConstraintID, DecisionVariable, Evaluate, Function,
    NamedFunction, RemovedConstraint, VariableID, VariableIDSet,
};
use std::collections::BTreeMap;

/// A constraint type capability flag for non-standard constraint types.
///
/// Standard constraints (`f(x) = 0` or `f(x) <= 0`) are always supported by all adapters
/// and do not need a capability flag. This enum only lists capabilities that adapters
/// must explicitly opt in to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdditionalCapability {
    /// Indicator constraints: binvar = 1 → f(x) <= 0
    Indicator,
}

/// Error returned when an Instance contains unsupported constraint types.
#[derive(Debug, Clone, thiserror::Error)]
pub struct UnsupportedCapabilities {
    pub unsupported: Vec<AdditionalCapability>,
}

impl std::fmt::Display for UnsupportedCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unsupported constraint types: {:?}", self.unsupported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Sense {
    #[default]
    Minimize,
    Maximize,
}

/// Instance, represents a mathematical optimization problem.
///
/// # Multi-type constraint architecture
///
/// Instance holds multiple [`ConstraintCollection`]s, one per constraint type:
/// - [`Constraint`]: standard constraints (`f(x) = 0` or `f(x) <= 0`)
/// - [`IndicatorConstraint`]: indicator constraints (`binvar = 1 → f(x) <= 0`)
///
/// Future constraint types (Disjunction, SOS1, OneHot, etc.) follow the same pattern:
/// add a new `ConstraintCollection<NewType>` field. See [`crate::constraint_type::ConstraintType`] for details.
///
/// Each constraint type has its own independent [`ConstraintID`] space:
/// constraint ID 1 for a regular constraint and constraint ID 1 for an indicator constraint
/// are distinct and do not conflict. Uniqueness is only required within the same type
/// (i.e. active and removed constraints of the same type must have disjoint IDs).
///
/// Adapter compatibility is checked via [`AdditionalCapability`] and [`Instance::check_capabilities`].
///
/// Invariants
/// -----------
/// - [`Self::decision_variables`] contains all decision variables used in the problem.
/// - The keys of [`Self::constraints`] and [`Self::removed_constraints`] are disjoint sets.
/// - The keys of [`Self::decision_variable_dependency`] must be in [`Self::decision_variables`],
///   but must NOT be used in the objective function or constraints.
///   These are "dependent variables" whose values are computed from other variables.
///   See also the document of [`DecisionVariableAnalysis`].
/// - The following three sets must be pairwise disjoint (from [`DecisionVariableAnalysis`]):
///   - **used**: Variable IDs appearing in the objective function or constraints
///   - **fixed**: Variable IDs with `substituted_value` set
///   - **dependent**: Keys of `decision_variable_dependency`
/// - [`Self::removed_constraints`] may contain fixed or dependent variable IDs.
///   These are substituted when the constraint is restored via [`Self::restore_constraint`].
/// - The keys of [`Self::named_functions`] match the `id()` of their values.
/// - [`Self::named_functions`] may contain fixed or dependent variable IDs (like `removed_constraints`).
///   Variable IDs in `named_functions` must be registered in [`Self::decision_variables`],
///   but are NOT included in the "used" set calculation.
///
#[derive(Debug, Clone, PartialEq, getset::Getters, getset::CopyGetters, Default)]
pub struct Instance {
    #[getset(get_copy = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    objective: Function,
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, DecisionVariable>,

    /// Regular constraints collection (active + removed).
    constraint_collection: ConstraintCollection<Constraint>,

    /// Indicator constraints collection (active + removed).
    indicator_constraint_collection: ConstraintCollection<IndicatorConstraint>,

    #[getset(get = "pub")]
    decision_variable_dependency: AcyclicAssignments,
    #[getset(get = "pub")]
    named_functions: BTreeMap<NamedFunctionID, NamedFunction>,

    /// The constraint hints, i.e. some constraints are in form of one-hot, SOS1,2, or other special types.
    ///
    /// Note
    /// -----
    /// This struct does not validate the hints in mathematical sense.
    /// Only checks the decision variable and constraint IDs are valid.
    #[getset(get = "pub")]
    constraint_hints: ConstraintHints,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub parameters: Option<v1::Parameters>,
    pub description: Option<v1::instance::Description>,
}

impl Instance {
    /// Active constraints.
    pub fn constraints(&self) -> &BTreeMap<ConstraintID, Constraint> {
        self.constraint_collection.active()
    }

    /// Removed constraints.
    pub fn removed_constraints(&self) -> &BTreeMap<ConstraintID, RemovedConstraint> {
        self.constraint_collection.removed()
    }

    /// The full constraint collection (active + removed).
    pub fn constraint_collection(&self) -> &ConstraintCollection<Constraint> {
        &self.constraint_collection
    }

    /// Active indicator constraints.
    pub fn indicator_constraints(
        &self,
    ) -> &BTreeMap<crate::IndicatorConstraintID, IndicatorConstraint> {
        self.indicator_constraint_collection.active()
    }

    /// Removed indicator constraints.
    pub fn removed_indicator_constraints(
        &self,
    ) -> &BTreeMap<crate::IndicatorConstraintID, RemovedIndicatorConstraint> {
        self.indicator_constraint_collection.removed()
    }

    /// The full indicator constraint collection (active + removed).
    pub fn indicator_constraint_collection(&self) -> &ConstraintCollection<IndicatorConstraint> {
        &self.indicator_constraint_collection
    }

    /// Returns the set of non-standard constraint capabilities required by this instance.
    ///
    /// Only **active** constraints are considered. Removed (relaxed) constraints are excluded
    /// because they are not passed to solver adapters — adapters only need to handle
    /// constraint types that are actively part of the problem.
    pub fn required_capabilities(&self) -> fnv::FnvHashSet<AdditionalCapability> {
        let mut caps = fnv::FnvHashSet::default();
        if !self.indicator_constraint_collection.active().is_empty() {
            caps.insert(AdditionalCapability::Indicator);
        }
        caps
    }

    /// Check that the given supported capabilities cover all constraint types in this instance.
    ///
    /// Only active constraints are checked (see [`Self::required_capabilities`]).
    ///
    /// Returns an error listing unsupported constraint types if any are found.
    pub fn check_capabilities(
        &self,
        supported: &fnv::FnvHashSet<AdditionalCapability>,
    ) -> Result<(), UnsupportedCapabilities> {
        let required = self.required_capabilities();
        let unsupported: Vec<_> = required.difference(supported).copied().collect();
        if unsupported.is_empty() {
            Ok(())
        } else {
            Err(UnsupportedCapabilities { unsupported })
        }
    }
}

/// Optimization problem instance with parameters
///
/// Invariants
/// -----------
/// - [`Self::decision_variables`] and [`Self::parameters`] contains all decision variables and parameters used in the problem.
///   - This means every IDs appearing in the constraints and the objective function must be included in either of them.
///   - The IDs of [`Self::decision_variables`] and [`Self::parameters`] are disjoint sets.
/// - The keys of [`Self::constraints`] and [`Self::removed_constraints`] are disjoint sets.
/// - The keys of [`Self::decision_variable_dependency`] must be in [`Self::decision_variables`],
///   but must NOT be used in the objective function or constraints.
///   See also the document of [`DecisionVariableAnalysis`].
/// - The following three sets must be pairwise disjoint (from [`DecisionVariableAnalysis`]):
///   - **used**: Variable IDs appearing in the objective function or constraints
///   - **fixed**: Variable IDs with `substituted_value` set
///   - **dependent**: Keys of `decision_variable_dependency`
/// - The keys of [`Self::named_functions`] match the `id()` of their values.
/// - [`Self::named_functions`] may contain fixed or dependent variable IDs (like `removed_constraints`).
///   Variable IDs in `named_functions` must be registered in [`Self::decision_variables`],
///   but are NOT included in the "used" set calculation.
///
#[derive(Debug, Clone, PartialEq, getset::Getters, Default)]
pub struct ParametricInstance {
    #[getset(get = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    objective: Function,
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, DecisionVariable>,
    #[getset(get = "pub")]
    parameters: BTreeMap<VariableID, v1::Parameter>,

    /// Regular constraints collection (active + removed).
    constraint_collection: ConstraintCollection<Constraint>,

    #[getset(get = "pub")]
    decision_variable_dependency: AcyclicAssignments,
    #[getset(get = "pub")]
    named_functions: BTreeMap<NamedFunctionID, NamedFunction>,

    /// The constraint hints, i.e. some constraints are in form of one-hot, SOS1,2, or other special types.
    ///
    /// Note
    /// -----
    /// This struct does not validate the hints in mathematical sense.
    /// Only checks the decision variable and constraint IDs are valid.
    #[getset(get = "pub")]
    constraint_hints: ConstraintHints,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub description: Option<v1::instance::Description>,
}

impl ParametricInstance {
    /// Active constraints.
    pub fn constraints(&self) -> &BTreeMap<ConstraintID, Constraint> {
        self.constraint_collection.active()
    }

    /// Removed constraints.
    pub fn removed_constraints(&self) -> &BTreeMap<ConstraintID, RemovedConstraint> {
        self.constraint_collection.removed()
    }
}
