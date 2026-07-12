mod analysis;
mod approx;
pub(crate) mod arbitrary;
mod builder;
mod clip_bounds;
mod convert;
mod decision_variable;
mod encoding;
mod evaluate;
mod format_function;
mod indicator;
mod log_encode;
mod logical_memory;
mod named_function;
mod new;
mod one_hot;
mod parametric_builder;
mod parse;
mod pass;
mod penalty;
mod qubo;
mod reduce_binary_power;
mod reduction;
mod serialize;
mod setter;
mod slack;
mod sos1;
mod stats;
mod substitute;
mod unary_encode;

pub use analysis::*;
pub use arbitrary::{InstanceParameters, InstanceSpace};
pub use builder::*;
pub use parametric_builder::*;
pub use stats::*;

use crate::{
    constraint::{ConstraintContextStore, RemovedReason},
    constraint_type::ConstraintCollection,
    decision_variable::{DecisionVariableTable, VariableLabelStore},
    indicator_constraint::IndicatorConstraint,
    named_function::NamedFunctionID,
    one_hot_constraint::OneHotConstraint,
    parameter::ParameterTable,
    sos1_constraint::Sos1Constraint,
    v1, AcyclicAssignments, Constraint, ConstraintContext, ConstraintID, DecisionVariable,
    Evaluate, Function, ModelingLabel, NamedFunction, NamedFunctionTable, VariableID,
    VariableIDSet,
};
use std::collections::{BTreeMap, HashMap};

/// A constraint type capability flag for non-standard constraint types.
///
/// Standard constraints (`f(x) = 0` or `f(x) <= 0`) are always supported by all adapters
/// and do not need a capability flag. This enum only lists capabilities that adapters
/// must explicitly opt in to.
///
/// The [`PartialOrd`] / [`Ord`] derives follow variant declaration order
/// (`Indicator < OneHot < Sos1`), which is also the order in which
/// [`Capabilities`] iterates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdditionalCapability {
    /// Indicator constraints: binvar = 1 → f(x) <= 0
    Indicator,
    /// One-hot constraints: exactly one of a set of binary variables must be 1
    OneHot,
    /// SOS1 constraints: at most one of a set of variables can be non-zero
    Sos1,
}

/// A set of [`AdditionalCapability`] flags.
///
/// Always represented as a [`std::collections::BTreeSet`] so iteration,
/// formatting, and comparison are deterministic and sorted by variant order.
pub type Capabilities = std::collections::BTreeSet<AdditionalCapability>;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    crate::logical_memory::LogicalMemoryProfile,
)]
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
/// Adapter compatibility is expressed via [`AdditionalCapability`]. Callers can read
/// [`Instance::required_capabilities`] to see which non-standard types the instance
/// carries, and use [`Instance::reduce_capabilities`] to convert unsupported types
/// into regular constraints.
///
/// # Mathematical operations
///
/// `Instance` is the root owner for operations whose meaning depends on more
/// than one table. Algebraically, an instance is an optimization problem over
/// one decision-variable space together with objective, constraint families,
/// named functions, assignment state, modeling labels, and provenance.
///
/// The following operations are therefore `Instance` operations, even when
/// their implementation eventually changes only one row in one internal table:
///
/// - expression-algebra actions such as substitution, partial evaluation, and
///   binary-power reduction;
/// - variable-space extensions such as slack-variable introduction;
/// - constraint-family morphisms such as one-hot, indicator, and SOS1
///   conversion into regular constraints;
/// - lifecycle actions such as relax and restore;
/// - unit propagation as a rewrite system over constraints plus assignment
///   state;
/// - modeling-context and provenance pushforward from source rows to generated
///   rows.
///
/// Lower-level tables and collections only apply the table-local row effects
/// requested by these root operations: fresh insertion, lifecycle-preserving
/// replacement, active-to-removed movement, restore through a host-supplied
/// normalizer, label/context updates for owned IDs, or by-value rewrites. They
/// do not decide whether an expression rewrite, variable-space extension,
/// family morphism, or restore is semantically valid.
///
/// # Invariants
///
/// - [`Self::decision_variables`] owns the
///   [`DecisionVariableTable`]: row IDs, decision-variable modeling
///   labels, and fixed values share one table owner.
/// - The decision-variable table rejects labels or fixed values for unknown
///   variable IDs, and fixed values must satisfy the corresponding row's
///   kind/bound.
/// - The keys of [`Self::constraints`] and [`Self::removed_constraints`] are disjoint sets.
/// - The keys of [`Self::decision_variable_dependency`] must be in [`Self::decision_variables`],
///   but must NOT be used in the objective function or constraints.
///   These are "dependent variables" whose values are computed from other variables.
/// - Decision variables are classified into mutually exclusive roles:
///   - **used**: Variable IDs appearing in the objective function or active constraints
///   - **fixed**: Variable IDs present in [`Self::fixed_decision_variable_values`] and not used
///   - **dependent**: Keys of `decision_variable_dependency` that are not used or fixed
/// - [`DecisionVariableUsage`] is the reverse-usage index for used decision variables only.
/// - [`Self::removed_constraints`] may contain fixed or dependent variable IDs.
///   These are substituted when the constraint is restored via [`Self::restore_constraint`].
/// - [`Self::named_functions`] is keyed by the table-owned
///   [`NamedFunctionID`]; named-function rows do not carry IDs.
/// - [`Self::named_functions`] may contain fixed or dependent variable IDs (like `removed_constraints`).
///   Variable IDs referenced by named functions must be registered in [`Self::decision_variables`],
///   but are NOT included in the "used" set calculation.
/// - Modeling-label and constraint-context sidecars are owned by their
///   corresponding top-level table or collection; every label/context ID must
///   refer to an existing decision variable, named function, or active/removed
///   constraint in that owner.
/// - Fixed decision-variable values are owned by
///   [`DecisionVariableTable`], not by individual [`DecisionVariable`]
///   values. The root [`Instance`] owns the host-level invariant that fixed
///   IDs are disjoint from solver-used and dependent variables.
///
/// ## Special-constraint invariants
///
/// Active and removed indicator / one-hot / SOS1 constraints are subject to the
/// same id-resolution rules as regular constraints, and additionally:
///
/// - [`Self::indicator_constraints`] and [`Self::removed_indicator_constraints`]
///   are disjoint sets; same for the one-hot and SOS1 collections (within each
///   kind — id spaces across kinds are independent).
/// - For every active or removed [`crate::IndicatorConstraint`], the
///   `indicator_variable` is registered in [`Self::decision_variables`] and
///   has [`Kind::Binary`](crate::decision_variable::Kind). The function body
///   may reference any defined variable.
/// - For every active or removed [`crate::OneHotConstraint`], every member of
///   `variables` is registered in [`Self::decision_variables`] and has
///   [`Kind::Binary`](crate::decision_variable::Kind).
/// - For every active or removed [`crate::Sos1Constraint`], `variables` is
///   non-empty and every member is registered in [`Self::decision_variables`].
///   SOS1 does not require [`Kind::Binary`](crate::decision_variable::Kind).
///
/// These invariants are enforced at every construction or mutation entry
/// point: [`Instance::builder`] (which accepts active maps for all three
/// kinds, a removed map for regular and indicator constraints, and validates
/// each), the post-construction setters [`Self::insert_constraint`] /
/// [`Self::add_constraint`] / [`Self::add_indicator_constraint`] /
/// [`Self::add_one_hot_constraint`] / [`Self::add_sos1_constraint`], and the
/// internal `relax_constraint` / `relax_indicator_constraint` /
/// `convert_all_one_hots_to_constraints` / `convert_all_sos1_to_constraints`
/// paths that populate the removed maps. Constraint-family storage is mutated
/// through operation-level collection primitives so active/removed
/// disjointness, removed reasons, and context sidecars remain attached to
/// owned row IDs.
///
#[derive(
    Debug,
    Clone,
    PartialEq,
    getset::Getters,
    getset::CopyGetters,
    Default,
    crate::logical_memory::LogicalMemoryProfile,
)]
pub struct Instance {
    #[getset(get_copy = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    objective: Function,
    /// Created decision-variable rows, modeling labels, and fixed values.
    decision_variables: DecisionVariableTable,

    /// Regular constraints collection (active + removed).
    constraint_collection: ConstraintCollection<Constraint>,

    /// Indicator constraints collection (active + removed).
    indicator_constraint_collection: ConstraintCollection<IndicatorConstraint>,

    /// One-hot constraints collection (active + removed).
    one_hot_constraint_collection: ConstraintCollection<OneHotConstraint>,

    /// SOS1 constraints collection (active + removed).
    sos1_constraint_collection: ConstraintCollection<Sos1Constraint>,

    #[getset(get = "pub")]
    decision_variable_dependency: AcyclicAssignments,
    /// Named-function rows plus their modeling labels.
    named_functions: NamedFunctionTable<NamedFunction>,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub parameters: Option<v1::Parameters>,
    pub description: Option<v1::instance::Description>,
    /// User-defined or third-party extension annotations.
    ///
    /// OMMX-reserved metadata is represented by explicit fields such as
    /// [`Self::description`].
    pub annotations: HashMap<String, String>,
}

impl Instance {
    /// Access the decision-variable definition table.
    pub fn decision_variable_table(&self) -> &DecisionVariableTable {
        &self.decision_variables
    }

    /// Access decision-variable rows keyed by table-owned IDs.
    pub fn decision_variables(&self) -> &BTreeMap<VariableID, DecisionVariable> {
        self.decision_variables.entries()
    }

    /// Access the per-variable modeling-label store.
    pub fn variable_labels(&self) -> &VariableLabelStore {
        self.decision_variables.labels()
    }

    /// Replace the modeling label for a decision variable owned by this instance.
    pub fn set_variable_label(
        &mut self,
        id: VariableID,
        label: ModelingLabel,
    ) -> crate::Result<()> {
        self.decision_variables.set_label(id, label)
    }

    /// Access table-owned fixed decision-variable values.
    pub fn fixed_decision_variable_values(&self) -> &BTreeMap<VariableID, f64> {
        self.decision_variables.fixed_values()
    }

    /// Return the fixed value for one decision variable, if it is fixed.
    pub fn fixed_decision_variable_value(&self, id: VariableID) -> Option<f64> {
        self.decision_variables.fixed_value(id)
    }

    /// Access named-function rows plus their modeling labels.
    pub fn named_function_table(&self) -> &NamedFunctionTable<NamedFunction> {
        &self.named_functions
    }

    /// Access named-function row payloads keyed by table-owned IDs.
    pub fn named_functions(&self) -> &BTreeMap<NamedFunctionID, NamedFunction> {
        self.named_functions.entries()
    }

    /// Access the per-named-function modeling-label store.
    pub fn named_function_labels(&self) -> &crate::named_function::NamedFunctionLabelStore {
        self.named_functions.labels()
    }

    /// Replace the modeling label for a named function owned by this instance.
    pub fn set_named_function_label(
        &mut self,
        id: NamedFunctionID,
        label: ModelingLabel,
    ) -> crate::Result<()> {
        self.named_functions.set_label(id, label)
    }

    /// Active constraints.
    pub fn constraints(&self) -> &BTreeMap<ConstraintID, Constraint> {
        self.constraint_collection.active()
    }

    /// Removed constraints.
    pub fn removed_constraints(&self) -> &BTreeMap<ConstraintID, (Constraint, RemovedReason)> {
        self.constraint_collection.removed()
    }

    /// The full constraint collection (active + removed).
    pub fn constraint_collection(&self) -> &ConstraintCollection<Constraint> {
        &self.constraint_collection
    }

    /// Access the per-constraint context store.
    pub fn constraint_context(&self) -> &ConstraintContextStore<ConstraintID> {
        self.constraint_collection.context()
    }

    /// Replace the context for a regular constraint owned by this instance.
    pub fn set_constraint_context(
        &mut self,
        id: ConstraintID,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        self.constraint_collection
            .set_context_for_owner(id, context, "constraint")
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
    ) -> &BTreeMap<crate::IndicatorConstraintID, (IndicatorConstraint, RemovedReason)> {
        self.indicator_constraint_collection.removed()
    }

    /// The full indicator constraint collection (active + removed).
    pub fn indicator_constraint_collection(&self) -> &ConstraintCollection<IndicatorConstraint> {
        &self.indicator_constraint_collection
    }

    /// Access the per-indicator-constraint context store.
    pub fn indicator_constraint_context(
        &self,
    ) -> &ConstraintContextStore<crate::IndicatorConstraintID> {
        self.indicator_constraint_collection.context()
    }

    /// Replace the context for an indicator constraint owned by this instance.
    pub fn set_indicator_constraint_context(
        &mut self,
        id: crate::IndicatorConstraintID,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        self.indicator_constraint_collection.set_context_for_owner(
            id,
            context,
            "indicator constraint",
        )
    }

    /// Active one-hot constraints.
    pub fn one_hot_constraints(&self) -> &BTreeMap<crate::OneHotConstraintID, OneHotConstraint> {
        self.one_hot_constraint_collection.active()
    }

    /// Removed one-hot constraints.
    pub fn removed_one_hot_constraints(
        &self,
    ) -> &BTreeMap<crate::OneHotConstraintID, (OneHotConstraint, RemovedReason)> {
        self.one_hot_constraint_collection.removed()
    }

    /// The full one-hot constraint collection (active + removed).
    pub fn one_hot_constraint_collection(&self) -> &ConstraintCollection<OneHotConstraint> {
        &self.one_hot_constraint_collection
    }

    /// Access the per-one-hot-constraint context store.
    pub fn one_hot_constraint_context(&self) -> &ConstraintContextStore<crate::OneHotConstraintID> {
        self.one_hot_constraint_collection.context()
    }

    /// Replace the context for a one-hot constraint owned by this instance.
    pub fn set_one_hot_constraint_context(
        &mut self,
        id: crate::OneHotConstraintID,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        self.one_hot_constraint_collection
            .set_context_for_owner(id, context, "one-hot constraint")
    }

    /// Active SOS1 constraints.
    pub fn sos1_constraints(&self) -> &BTreeMap<crate::Sos1ConstraintID, Sos1Constraint> {
        self.sos1_constraint_collection.active()
    }

    /// Removed SOS1 constraints.
    pub fn removed_sos1_constraints(
        &self,
    ) -> &BTreeMap<crate::Sos1ConstraintID, (Sos1Constraint, RemovedReason)> {
        self.sos1_constraint_collection.removed()
    }

    /// The full SOS1 constraint collection (active + removed).
    pub fn sos1_constraint_collection(&self) -> &ConstraintCollection<Sos1Constraint> {
        &self.sos1_constraint_collection
    }

    /// Access the per-SOS1-constraint context store.
    pub fn sos1_constraint_context(&self) -> &ConstraintContextStore<crate::Sos1ConstraintID> {
        self.sos1_constraint_collection.context()
    }

    /// Replace the context for an SOS1 constraint owned by this instance.
    pub fn set_sos1_constraint_context(
        &mut self,
        id: crate::Sos1ConstraintID,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        self.sos1_constraint_collection
            .set_context_for_owner(id, context, "SOS1 constraint")
    }

    /// Returns the set of non-standard constraint capabilities required by this instance.
    ///
    /// Only **active** constraints are considered. Removed (relaxed) constraints are excluded
    /// because they are not passed to solver adapters — adapters only need to handle
    /// constraint types that are actively part of the problem.
    pub fn required_capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new();
        if !self.indicator_constraint_collection.active().is_empty() {
            caps.insert(AdditionalCapability::Indicator);
        }
        if !self.one_hot_constraint_collection.active().is_empty() {
            caps.insert(AdditionalCapability::OneHot);
        }
        if !self.sos1_constraint_collection.active().is_empty() {
            caps.insert(AdditionalCapability::Sos1);
        }
        caps
    }

    /// Convert constraint types not in `supported` into regular constraints.
    ///
    /// For every capability in `required_capabilities() - supported`, call the
    /// corresponding bulk conversion (`convert_all_indicators_to_constraints`,
    /// `convert_all_one_hots_to_constraints`, or `convert_all_sos1_to_constraints`).
    /// After this call, the instance's [`Self::required_capabilities`] is a
    /// subset of `supported`.
    ///
    /// Returns the set of capabilities that were actually converted. Iteration
    /// order follows [`Capabilities`]'s sorted order (`Indicator`, `OneHot`,
    /// `Sos1`). The set is empty when nothing needed conversion. Each
    /// conversion is also emitted as an `INFO`-level [`tracing`] event inside
    /// the `reduce_capabilities` span (target `ommx::instance`) so it surfaces
    /// through `pyo3-tracing-opentelemetry` on the Python side.
    ///
    /// Errors if any underlying conversion fails (e.g. SOS1 / indicator with
    /// non-finite bounds). Each per-type conversion is atomic, but this method
    /// is **not** atomic across types: earlier conversions are not rolled back
    /// if a later one fails. Callers that need cross-type atomicity should
    /// validate / clone up front.
    #[tracing::instrument(skip_all)]
    pub fn reduce_capabilities(&mut self, supported: &Capabilities) -> crate::Result<Capabilities> {
        let mut converted = Capabilities::new();
        // Iterate in a fixed order so logs / callers see deterministic output.
        for cap in [
            AdditionalCapability::Indicator,
            AdditionalCapability::OneHot,
            AdditionalCapability::Sos1,
        ] {
            if supported.contains(&cap) {
                continue;
            }
            let converted_any = match cap {
                AdditionalCapability::Indicator => {
                    if self.indicator_constraint_collection.active().is_empty() {
                        false
                    } else {
                        self.convert_all_indicators_to_constraints()?;
                        true
                    }
                }
                AdditionalCapability::OneHot => {
                    if self.one_hot_constraint_collection.active().is_empty() {
                        false
                    } else {
                        self.convert_all_one_hots_to_constraints()?;
                        true
                    }
                }
                AdditionalCapability::Sos1 => {
                    if self.sos1_constraint_collection.active().is_empty() {
                        false
                    } else {
                        self.convert_all_sos1_to_constraints()?;
                        true
                    }
                }
            };
            if converted_any {
                tracing::info!(
                    "reduce_capabilities: {cap:?} is not in supported capabilities; converted to regular constraints"
                );
                converted.insert(cap);
            }
        }
        Ok(converted)
    }
}

/// Optimization problem instance with parameters
///
/// # Mathematical operations
///
/// `ParametricInstance` owns the same root operations as [`Instance`] and also
/// owns the parameter-specialization operation. Parameter IDs share the
/// [`VariableID`] namespace with decision variables, so only the enclosing
/// parametric instance can interpret an expression reference as a decision
/// variable or a parameter.
///
/// [`Self::with_parameters`] applies a parameter assignment to produce a
/// concrete [`Instance`]. This is a root operation: it substitutes parameter
/// values out of every expression-bearing component while preserving structural
/// constraint families whose member IDs must already be real decision
/// variables.
///
/// # Invariants
///
/// - [`Self::decision_variables`] owns the
///   [`DecisionVariableTable`]: row IDs, decision-variable modeling
///   labels, and fixed values share one table owner.
/// - The decision-variable table rejects labels or fixed values for unknown
///   variable IDs, and fixed values must satisfy the corresponding row's
///   kind/bound.
/// - [`Self::parameters`] owns the parameter ID universe and parameter
///   modeling labels through [`ParameterTable`]. Parameter IDs intentionally
///   use [`VariableID`] rather than a separate `ParameterID`, because
///   algebraic expressions cannot distinguish decision-variable references
///   from parameter references without the enclosing root.
/// - [`Self::decision_variables`] and [`Self::parameters`] together contain
///   every ID that may appear in the objective, regular/indicator constraint
///   bodies, named functions, and dependency RHS expressions.
/// - The IDs of [`Self::decision_variables`] and [`Self::parameters`] are
///   disjoint sets. This shared-namespace invariant is host-level state and
///   is validated by [`ParametricInstance::builder`] / protobuf parsing, not
///   by [`ParameterTable`] alone.
/// - The keys of [`Self::constraints`] and [`Self::removed_constraints`] are disjoint sets.
/// - The keys of [`Self::decision_variable_dependency`] must be in [`Self::decision_variables`],
///   but must NOT be used in the objective function or constraints.
///   The RHS expressions of [`Self::decision_variable_dependency`] may
///   reference IDs from [`Self::decision_variables`] or [`Self::parameters`],
///   and may not reference undefined IDs. Parameter IDs in RHS expressions are
///   evaluated by [`Self::with_parameters`].
/// - Decision variables are classified into mutually exclusive roles:
///   - **used**: Variable IDs appearing in the objective function or active constraints
///   - **fixed**: Variable IDs present in [`Self::fixed_decision_variable_values`] and not used
///   - **dependent**: Keys of `decision_variable_dependency` that are not used or fixed
/// - [`DecisionVariableUsage`] is the reverse-usage index for used decision variables only.
/// - [`Self::named_functions`] is keyed by the table-owned
///   [`NamedFunctionID`]; named-function rows do not carry IDs.
/// - [`Self::named_functions`] may contain fixed or dependent decision-variable
///   IDs (like `removed_constraints`) and may also reference parameter IDs.
///   Every referenced ID must be registered in either
///   [`Self::decision_variables`] or [`Self::parameters`], but decision-variable
///   IDs appearing only in named functions are NOT included in the "used" set
///   calculation.
/// - Modeling-label and constraint-context sidecars are owned by their
///   corresponding top-level table or collection; every label/context ID must
///   refer to an existing decision variable, parameter, named function, or
///   active/removed constraint in that owner. Parameter labels are owned by
///   [`ParameterTable`]; parameter IDs are not valid variable-label IDs.
/// - Fixed decision-variable values are owned by
///   [`DecisionVariableTable`]. The root [`ParametricInstance`] owns
///   the host-level invariant that fixed IDs are disjoint from solver-used and
///   dependent variables, and from parameter IDs via the shared namespace rule.
///
/// ## Special-constraint invariants
///
/// The same special-constraint invariants apply as on [`Instance`], with one
/// key difference: function bodies in indicator and regular constraints may
/// reference parameter IDs (which are substituted via
/// [`Self::with_parameters`]), but **structural** variable positions cannot
/// — they must be real decision variables. Specifically:
///
/// - For every active or removed [`crate::IndicatorConstraint`], the
///   `indicator_variable` is registered in [`Self::decision_variables`]
///   (not [`Self::parameters`]) and has
///   [`Kind::Binary`](crate::decision_variable::Kind). The function body
///   may reference any defined variable or parameter.
/// - For every active or removed [`crate::OneHotConstraint`], every member of
///   `variables` is registered in [`Self::decision_variables`] and has
///   [`Kind::Binary`](crate::decision_variable::Kind).
/// - For every active or removed [`crate::Sos1Constraint`], `variables` is
///   non-empty and every member is registered in [`Self::decision_variables`].
/// - The constraint id-disjointness invariants (active vs. removed, per kind)
///   match [`Instance`].
///
/// These invariants are enforced at every construction or mutation entry
/// point: [`ParametricInstance::builder`] (which mirrors
/// [`Instance::builder`] and accepts active maps for all three kinds, plus
/// removed maps for regular and indicator constraints, validating each),
/// and the post-construction setters [`Self::add_constraint`] /
/// [`Self::add_indicator_constraint`] / [`Self::add_one_hot_constraint`] /
/// [`Self::add_sos1_constraint`].
///
/// [`Self::with_parameters`] partially evaluates parameter IDs out of every
/// expression that could contain one when materializing a parametric
/// instance into an [`Instance`]: the objective, active and removed regular
/// constraint bodies, active and removed indicator constraint function
/// bodies, named functions, and `decision_variable_dependency` RHS
/// expressions. OneHot/SOS1 collections (active and removed) pass through
/// unchanged because their variable sets are required to be real decision
/// variables at construction time. The resulting [`Instance`] satisfies its
/// own (stricter) invariants — no parameter IDs survive anywhere.
///
#[derive(Debug, Clone, PartialEq, getset::Getters, Default)]
pub struct ParametricInstance {
    #[getset(get = "pub")]
    sense: Sense,
    #[getset(get = "pub")]
    objective: Function,
    /// Created decision-variable rows, modeling labels, and fixed values.
    decision_variables: DecisionVariableTable,
    #[getset(get = "pub")]
    parameters: ParameterTable,

    /// Regular constraints collection (active + removed).
    constraint_collection: ConstraintCollection<Constraint>,

    /// Indicator constraints collection (active + removed).
    indicator_constraint_collection: ConstraintCollection<IndicatorConstraint>,

    /// One-hot constraints collection (active + removed).
    one_hot_constraint_collection: ConstraintCollection<OneHotConstraint>,

    /// SOS1 constraints collection (active + removed).
    sos1_constraint_collection: ConstraintCollection<Sos1Constraint>,

    #[getset(get = "pub")]
    decision_variable_dependency: AcyclicAssignments,
    /// Named-function rows plus their modeling labels.
    named_functions: NamedFunctionTable<NamedFunction>,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub description: Option<v1::instance::Description>,
    /// User-defined or third-party extension annotations.
    ///
    /// OMMX-reserved metadata is represented by explicit fields such as
    /// [`Self::description`].
    pub annotations: HashMap<String, String>,
}

impl ParametricInstance {
    /// Access the decision-variable definition table.
    pub fn decision_variable_table(&self) -> &DecisionVariableTable {
        &self.decision_variables
    }

    /// Access decision-variable rows keyed by table-owned IDs.
    pub fn decision_variables(&self) -> &BTreeMap<VariableID, DecisionVariable> {
        self.decision_variables.entries()
    }

    /// Access the per-variable modeling-label store.
    pub fn variable_labels(&self) -> &VariableLabelStore {
        self.decision_variables.labels()
    }

    /// Replace the modeling label for a decision variable owned by this
    /// parametric instance.
    pub fn set_variable_label(
        &mut self,
        id: VariableID,
        label: ModelingLabel,
    ) -> crate::Result<()> {
        self.decision_variables.set_label(id, label)
    }

    /// Access table-owned fixed decision-variable values.
    pub fn fixed_decision_variable_values(&self) -> &BTreeMap<VariableID, f64> {
        self.decision_variables.fixed_values()
    }

    /// Return the fixed value for one decision variable, if it is fixed.
    pub fn fixed_decision_variable_value(&self, id: VariableID) -> Option<f64> {
        self.decision_variables.fixed_value(id)
    }

    /// Access named-function rows plus their modeling labels.
    pub fn named_function_table(&self) -> &NamedFunctionTable<NamedFunction> {
        &self.named_functions
    }

    /// Access named-function row payloads keyed by table-owned IDs.
    pub fn named_functions(&self) -> &BTreeMap<NamedFunctionID, NamedFunction> {
        self.named_functions.entries()
    }

    /// Access the per-named-function modeling-label store.
    pub fn named_function_labels(&self) -> &crate::named_function::NamedFunctionLabelStore {
        self.named_functions.labels()
    }

    /// Replace the modeling label for a named function owned by this parametric
    /// instance.
    pub fn set_named_function_label(
        &mut self,
        id: NamedFunctionID,
        label: ModelingLabel,
    ) -> crate::Result<()> {
        self.named_functions.set_label(id, label)
    }

    /// Active constraints.
    pub fn constraints(&self) -> &BTreeMap<ConstraintID, Constraint> {
        self.constraint_collection.active()
    }

    /// Removed constraints.
    pub fn removed_constraints(&self) -> &BTreeMap<ConstraintID, (Constraint, RemovedReason)> {
        self.constraint_collection.removed()
    }

    /// The full constraint collection (active + removed).
    pub fn constraint_collection(&self) -> &ConstraintCollection<Constraint> {
        &self.constraint_collection
    }

    /// Access the per-constraint context store.
    pub fn constraint_context(&self) -> &ConstraintContextStore<ConstraintID> {
        self.constraint_collection.context()
    }

    /// Replace the context for a regular constraint owned by this parametric
    /// instance.
    pub fn set_constraint_context(
        &mut self,
        id: ConstraintID,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        self.constraint_collection
            .set_context_for_owner(id, context, "constraint")
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
    ) -> &BTreeMap<crate::IndicatorConstraintID, (IndicatorConstraint, RemovedReason)> {
        self.indicator_constraint_collection.removed()
    }

    /// The full indicator constraint collection.
    pub fn indicator_constraint_collection(&self) -> &ConstraintCollection<IndicatorConstraint> {
        &self.indicator_constraint_collection
    }

    /// Access the per-indicator-constraint context store.
    pub fn indicator_constraint_context(
        &self,
    ) -> &ConstraintContextStore<crate::IndicatorConstraintID> {
        self.indicator_constraint_collection.context()
    }

    /// Replace the context for an indicator constraint owned by this parametric
    /// instance.
    pub fn set_indicator_constraint_context(
        &mut self,
        id: crate::IndicatorConstraintID,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        self.indicator_constraint_collection.set_context_for_owner(
            id,
            context,
            "indicator constraint",
        )
    }

    /// Active one-hot constraints.
    pub fn one_hot_constraints(&self) -> &BTreeMap<crate::OneHotConstraintID, OneHotConstraint> {
        self.one_hot_constraint_collection.active()
    }

    /// Removed one-hot constraints.
    pub fn removed_one_hot_constraints(
        &self,
    ) -> &BTreeMap<crate::OneHotConstraintID, (OneHotConstraint, RemovedReason)> {
        self.one_hot_constraint_collection.removed()
    }

    /// The full one-hot constraint collection.
    pub fn one_hot_constraint_collection(&self) -> &ConstraintCollection<OneHotConstraint> {
        &self.one_hot_constraint_collection
    }

    /// Access the per-one-hot-constraint context store.
    pub fn one_hot_constraint_context(&self) -> &ConstraintContextStore<crate::OneHotConstraintID> {
        self.one_hot_constraint_collection.context()
    }

    /// Replace the context for a one-hot constraint owned by this parametric
    /// instance.
    pub fn set_one_hot_constraint_context(
        &mut self,
        id: crate::OneHotConstraintID,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        self.one_hot_constraint_collection
            .set_context_for_owner(id, context, "one-hot constraint")
    }

    /// Active SOS1 constraints.
    pub fn sos1_constraints(&self) -> &BTreeMap<crate::Sos1ConstraintID, Sos1Constraint> {
        self.sos1_constraint_collection.active()
    }

    /// Removed SOS1 constraints.
    pub fn removed_sos1_constraints(
        &self,
    ) -> &BTreeMap<crate::Sos1ConstraintID, (Sos1Constraint, RemovedReason)> {
        self.sos1_constraint_collection.removed()
    }

    /// The full SOS1 constraint collection.
    pub fn sos1_constraint_collection(&self) -> &ConstraintCollection<Sos1Constraint> {
        &self.sos1_constraint_collection
    }

    /// Access the per-SOS1-constraint context store.
    pub fn sos1_constraint_context(&self) -> &ConstraintContextStore<crate::Sos1ConstraintID> {
        self.sos1_constraint_collection.context()
    }

    /// Replace the context for an SOS1 constraint owned by this parametric
    /// instance.
    pub fn set_sos1_constraint_context(
        &mut self,
        id: crate::Sos1ConstraintID,
        context: ConstraintContext,
    ) -> crate::Result<()> {
        self.sos1_constraint_collection
            .set_context_for_owner(id, context, "SOS1 constraint")
    }
}

#[cfg(test)]
mod reduce_capabilities_tests {
    use super::*;
    use crate::{
        indicator_constraint::{IndicatorConstraint, IndicatorConstraintID},
        linear,
        one_hot_constraint::{OneHotConstraint, OneHotConstraintID},
        sos1_constraint::{Sos1Constraint, Sos1ConstraintID},
        Bound, DecisionVariable, Equality, Function, Kind, VariableID,
    };
    use maplit::btreemap;
    use std::collections::{BTreeMap, BTreeSet};

    /// Build an instance with one of each non-standard constraint type, suitable
    /// for Big-M conversion (binary variables have bound [0, 1]).
    fn instance_with_all_capabilities() -> Instance {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::binary(),
            VariableID::from(1) => DecisionVariable::binary(),
            VariableID::from(2) => DecisionVariable::binary(),
            VariableID::from(3) => DecisionVariable::binary(),
        };
        let one_hot = OneHotConstraint::new(
            [VariableID::from(0), VariableID::from(1)]
                .into_iter()
                .collect(),
        )
        .unwrap();
        let sos1 = Sos1Constraint::new(
            [VariableID::from(2), VariableID::from(3)]
                .into_iter()
                .collect(),
        )
        .unwrap();
        // Indicator: y=1 => x0 <= 0 (trivially satisfied since x0 in [0,1], upper=1>0 emits upper Big-M)
        let indicator = IndicatorConstraint::new(
            VariableID::from(1),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(0)),
        );
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(1),
                indicator,
            )]))
            .one_hot_constraints(BTreeMap::from([(OneHotConstraintID::from(1), one_hot)]))
            .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(1), sos1)]))
            .build()
            .unwrap()
    }

    #[test]
    fn noop_when_all_required_are_supported() {
        // Every required capability is in `supported` → nothing converted,
        // instance left untouched.
        let mut instance = instance_with_all_capabilities();
        let supported: Capabilities = [
            AdditionalCapability::Indicator,
            AdditionalCapability::OneHot,
            AdditionalCapability::Sos1,
        ]
        .into_iter()
        .collect();
        let before_indicators = instance.indicator_constraints().clone();
        let before_one_hots = instance.one_hot_constraints().clone();
        let before_sos1 = instance.sos1_constraints().clone();

        let converted = instance.reduce_capabilities(&supported).unwrap();

        assert!(converted.is_empty());
        assert_eq!(instance.indicator_constraints(), &before_indicators);
        assert_eq!(instance.one_hot_constraints(), &before_one_hots);
        assert_eq!(instance.sos1_constraints(), &before_sos1);
    }

    #[test]
    fn converts_only_unsupported_capabilities() {
        // Supported = {Sos1}: Indicator and OneHot must be converted, SOS1 kept.
        let mut instance = instance_with_all_capabilities();
        let supported: Capabilities = [AdditionalCapability::Sos1].into_iter().collect();

        let converted = instance.reduce_capabilities(&supported).unwrap();

        let expected: Capabilities = [
            AdditionalCapability::Indicator,
            AdditionalCapability::OneHot,
        ]
        .into_iter()
        .collect();
        assert_eq!(converted, expected);
        assert!(instance.indicator_constraints().is_empty());
        assert!(instance.one_hot_constraints().is_empty());
        assert!(!instance.sos1_constraints().is_empty());
        // required_capabilities is now a subset of supported.
        assert!(instance.required_capabilities().is_subset(&supported));
    }

    #[test]
    fn empty_supported_converts_everything() {
        // No capabilities supported → all three are converted.
        let mut instance = instance_with_all_capabilities();
        let supported = Capabilities::new();

        let converted = instance.reduce_capabilities(&supported).unwrap();

        let expected: Capabilities = [
            AdditionalCapability::Indicator,
            AdditionalCapability::OneHot,
            AdditionalCapability::Sos1,
        ]
        .into_iter()
        .collect();
        assert_eq!(converted, expected);
        assert!(instance.required_capabilities().is_empty());
    }

    #[test]
    fn skips_capabilities_that_are_not_required() {
        // Instance has only a OneHot. Empty `supported` → only OneHot is reported
        // as converted; Indicator / SOS1 aren't in the returned set since they
        // were never present to begin with.
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::binary(),
            VariableID::from(1) => DecisionVariable::binary(),
        };
        let one_hot = OneHotConstraint::new(
            [VariableID::from(0), VariableID::from(1)]
                .into_iter()
                .collect(),
        )
        .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(OneHotConstraintID::from(1), one_hot)]))
            .build()
            .unwrap();
        let supported = Capabilities::new();

        let converted = instance.reduce_capabilities(&supported).unwrap();

        let expected: Capabilities = [AdditionalCapability::OneHot].into_iter().collect();
        assert_eq!(converted, expected);
        assert!(instance.one_hot_constraints().is_empty());
    }

    #[test]
    fn conversion_failure_is_propagated() {
        // SOS1 over a continuous variable with infinite bound cannot be Big-M
        // converted; reduce_capabilities surfaces the underlying error.
        let dv = DecisionVariable::continuous();
        let sos1 = Sos1Constraint::new([VariableID::from(0)].into_iter().collect::<BTreeSet<_>>())
            .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(btreemap! { VariableID::from(0) => dv })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(1), sos1)]))
            .build()
            .unwrap();
        let supported = Capabilities::new();

        let err = instance.reduce_capabilities(&supported).unwrap_err();
        assert!(err.to_string().contains("non-finite"));
    }

    #[test]
    fn integer_sos1_converts_with_new_indicator() {
        // A single SOS1 over an integer variable with finite bound [-2, 3]:
        // reduce_capabilities should invoke the SOS1 Big-M conversion which
        // allocates a fresh binary indicator.
        let dv = DecisionVariable::new(
            Kind::Integer,
            Bound::new(-2.0, 3.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let sos1 = Sos1Constraint::new([VariableID::from(0)].into_iter().collect::<BTreeSet<_>>())
            .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(0)))
            .decision_variables(btreemap! { VariableID::from(0) => dv })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(Sos1ConstraintID::from(1), sos1)]))
            .build()
            .unwrap();

        let converted = instance.reduce_capabilities(&Capabilities::new()).unwrap();

        let expected: Capabilities = [AdditionalCapability::Sos1].into_iter().collect();
        assert_eq!(converted, expected);
        // Fresh binary indicator was allocated → decision variable count went up.
        assert_eq!(instance.decision_variables.len(), 2);
        assert!(instance.sos1_constraints().is_empty());
        assert!(!instance.constraints().is_empty());
    }
}
