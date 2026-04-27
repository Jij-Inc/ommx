mod analysis;
mod approx;
pub(crate) mod arbitrary;
mod builder;
mod clip_bounds;
mod convert;
mod decision_variable;
mod evaluate;
mod indicator;
mod log_encode;
mod logical_memory;
mod named_function;
mod new;
mod one_hot;
mod parametric_builder;
pub mod parse;
mod pass;
mod penalty;
mod qubo;
mod reduce_binary_power;
mod serialize;
mod setter;
mod slack;
mod sos1;
mod stats;
mod substitute;

pub use analysis::*;
pub use arbitrary::InstanceParameters;
pub use builder::*;
pub use parametric_builder::*;
pub use stats::*;

use crate::{
    constraint::RemovedReason, constraint_type::ConstraintCollection,
    decision_variable::VariableMetadataStore, indicator_constraint::IndicatorConstraint,
    named_function::NamedFunctionID, one_hot_constraint::OneHotConstraint,
    sos1_constraint::Sos1Constraint, v1, AcyclicAssignments, Constraint, ConstraintID,
    DecisionVariable, Evaluate, Function, NamedFunction, VariableID, VariableIDSet,
};
use std::collections::BTreeMap;

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
/// Adapter compatibility is expressed via [`AdditionalCapability`]. Callers can read
/// [`Instance::required_capabilities`] to see which non-standard types the instance
/// carries, and use [`Instance::reduce_capabilities`] to convert unsupported types
/// into regular constraints.
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
    #[getset(get = "pub")]
    decision_variables: BTreeMap<VariableID, DecisionVariable>,

    /// Per-variable auxiliary metadata (`name`, `subscripts`, `parameters`,
    /// `description`). Sibling field of [`Self::decision_variables`]; together
    /// they form the canonical decision-variable storage.
    variable_metadata: VariableMetadataStore,

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
    #[getset(get = "pub")]
    named_functions: BTreeMap<NamedFunctionID, NamedFunction>,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub parameters: Option<v1::Parameters>,
    pub description: Option<v1::instance::Description>,
}

impl Instance {
    /// Access the per-variable metadata store.
    pub fn variable_metadata(&self) -> &VariableMetadataStore {
        &self.variable_metadata
    }

    /// Mutable access to the per-variable metadata store.
    pub fn variable_metadata_mut(&mut self) -> &mut VariableMetadataStore {
        &mut self.variable_metadata
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

    /// Mutable access to the full constraint collection (active + removed).
    pub fn constraint_collection_mut(&mut self) -> &mut ConstraintCollection<Constraint> {
        &mut self.constraint_collection
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

    /// Mutable access to the full indicator constraint collection.
    pub fn indicator_constraint_collection_mut(
        &mut self,
    ) -> &mut ConstraintCollection<IndicatorConstraint> {
        &mut self.indicator_constraint_collection
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

    /// Mutable access to the full one-hot constraint collection.
    pub fn one_hot_constraint_collection_mut(
        &mut self,
    ) -> &mut ConstraintCollection<OneHotConstraint> {
        &mut self.one_hot_constraint_collection
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

    /// Mutable access to the full SOS1 constraint collection.
    pub fn sos1_constraint_collection_mut(&mut self) -> &mut ConstraintCollection<Sos1Constraint> {
        &mut self.sos1_constraint_collection
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

    /// Per-variable auxiliary metadata (sibling of [`Self::decision_variables`]).
    /// The (unrelated) parametric `parameters` field above stores
    /// per-id [`v1::Parameter`] data for parameterized instances and is
    /// independent from this metadata store.
    variable_metadata: VariableMetadataStore,

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
    #[getset(get = "pub")]
    named_functions: BTreeMap<NamedFunctionID, NamedFunction>,

    // Optional fields for additional metadata.
    // These fields are public since arbitrary values can be set without validation.
    pub description: Option<v1::instance::Description>,
}

impl ParametricInstance {
    /// Access the per-variable metadata store.
    pub fn variable_metadata(&self) -> &VariableMetadataStore {
        &self.variable_metadata
    }

    /// Mutable access to the per-variable metadata store.
    pub fn variable_metadata_mut(&mut self) -> &mut VariableMetadataStore {
        &mut self.variable_metadata
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

    /// Mutable access to the full constraint collection.
    pub fn constraint_collection_mut(&mut self) -> &mut ConstraintCollection<Constraint> {
        &mut self.constraint_collection
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

    /// Mutable access to the full indicator constraint collection.
    pub fn indicator_constraint_collection_mut(
        &mut self,
    ) -> &mut ConstraintCollection<IndicatorConstraint> {
        &mut self.indicator_constraint_collection
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

    /// Mutable access to the full one-hot constraint collection.
    pub fn one_hot_constraint_collection_mut(
        &mut self,
    ) -> &mut ConstraintCollection<OneHotConstraint> {
        &mut self.one_hot_constraint_collection
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

    /// Mutable access to the full SOS1 constraint collection.
    pub fn sos1_constraint_collection_mut(&mut self) -> &mut ConstraintCollection<Sos1Constraint> {
        &mut self.sos1_constraint_collection
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
            VariableID::from(0) => DecisionVariable::binary(VariableID::from(0)),
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };
        let one_hot = OneHotConstraint::new(
            [VariableID::from(0), VariableID::from(1)]
                .into_iter()
                .collect(),
        );
        let sos1 = Sos1Constraint::new(
            [VariableID::from(2), VariableID::from(3)]
                .into_iter()
                .collect(),
        );
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
            VariableID::from(0) => DecisionVariable::binary(VariableID::from(0)),
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
        };
        let one_hot = OneHotConstraint::new(
            [VariableID::from(0), VariableID::from(1)]
                .into_iter()
                .collect(),
        );
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
        let dv = DecisionVariable::continuous(VariableID::from(0));
        let sos1 = Sos1Constraint::new([VariableID::from(0)].into_iter().collect::<BTreeSet<_>>());
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
            VariableID::from(0),
            Kind::Integer,
            Bound::new(-2.0, 3.0).unwrap(),
            None,
            crate::ATol::default(),
        )
        .unwrap();
        let sos1 = Sos1Constraint::new([VariableID::from(0)].into_iter().collect::<BTreeSet<_>>());
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
