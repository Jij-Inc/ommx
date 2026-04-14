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
    constraint_hints::ConstraintHints, constraint_type::ConstraintCollection,
    named_function::NamedFunctionID, parse::Parse, v1, AcyclicAssignments, Constraint,
    ConstraintID, DecisionVariable, Evaluate, Function, NamedFunction, RemovedConstraint,
    VariableID, VariableIDSet,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Sense {
    #[default]
    Minimize,
    Maximize,
}

/// Instance, represents a mathematical optimization problem.
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
