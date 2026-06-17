use super::Instance;
use crate::{
    Bounds, ConstraintID, DecisionVariable, Evaluate, IndicatorConstraintID, Kind, NamedFunctionID,
    OneHotConstraintID, Sos1ConstraintID, VariableID, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

/// Role of a decision variable in an [`Instance`].
///
/// The role is derived from the current instance structure:
///
/// - [`DecisionVariableRole::Used`] means the variable appears in the objective
///   function or in an active constraint family that affects solver input.
/// - [`DecisionVariableRole::Fixed`] means the variable has
///   [`DecisionVariable::substituted_value`] and is not solver-used.
/// - [`DecisionVariableRole::Dependent`] means the variable is defined by
///   `decision_variable_dependency` and is neither solver-used nor fixed.
/// - [`DecisionVariableRole::Irrelevant`] is the remaining case.
///
/// Named functions are intentionally not solver-used. Their references are
/// still recorded on [`VariableUsage`] for diagnostics.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DecisionVariableRole {
    Used,
    Fixed,
    Dependent,
    Irrelevant,
}

impl DecisionVariableRole {
    pub fn as_str(self) -> &'static str {
        match self {
            DecisionVariableRole::Used => "used",
            DecisionVariableRole::Fixed => "fixed",
            DecisionVariableRole::Dependent => "dependent",
            DecisionVariableRole::Irrelevant => "irrelevant",
        }
    }
}

impl std::fmt::Display for DecisionVariableRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Reverse-usage snapshot for one decision variable.
///
/// This is intentionally a snapshot. It copies the substituted value at
/// construction time and records inverse references from a variable to the
/// instance components that use it. That makes [`DecisionVariableUsageCore`]
/// self-contained for role queries in bindings that cannot borrow an
/// [`Instance`] for the lifetime of the usage view.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VariableUsage {
    substituted_value: Option<f64>,
    role: DecisionVariableRole,
    used_in_objective: bool,
    used_in_regular_constraints: BTreeSet<ConstraintID>,
    used_in_indicator_constraints: BTreeSet<IndicatorConstraintID>,
    used_in_one_hot_constraints: BTreeSet<OneHotConstraintID>,
    used_in_sos1_constraints: BTreeSet<Sos1ConstraintID>,
    used_in_named_functions: BTreeSet<NamedFunctionID>,
    defines_dependent_variable: bool,
    used_in_dependency_rhs_of: VariableIDSet,
}

impl VariableUsage {
    fn new(dv: &DecisionVariable) -> Self {
        Self {
            substituted_value: dv.substituted_value(),
            role: DecisionVariableRole::Irrelevant,
            used_in_objective: false,
            used_in_regular_constraints: BTreeSet::new(),
            used_in_indicator_constraints: BTreeSet::new(),
            used_in_one_hot_constraints: BTreeSet::new(),
            used_in_sos1_constraints: BTreeSet::new(),
            used_in_named_functions: BTreeSet::new(),
            defines_dependent_variable: false,
            used_in_dependency_rhs_of: VariableIDSet::new(),
        }
    }

    fn refresh_role(&mut self) {
        self.role = if self.is_used_by_solver() {
            DecisionVariableRole::Used
        } else if self.substituted_value.is_some() {
            DecisionVariableRole::Fixed
        } else if self.defines_dependent_variable {
            DecisionVariableRole::Dependent
        } else {
            DecisionVariableRole::Irrelevant
        };
    }

    pub fn substituted_value(&self) -> Option<f64> {
        self.substituted_value
    }

    pub fn role(&self) -> DecisionVariableRole {
        self.role
    }

    pub fn used_in_objective(&self) -> bool {
        self.used_in_objective
    }

    pub fn used_in_regular_constraints(&self) -> &BTreeSet<ConstraintID> {
        &self.used_in_regular_constraints
    }

    pub fn used_in_indicator_constraints(&self) -> &BTreeSet<IndicatorConstraintID> {
        &self.used_in_indicator_constraints
    }

    pub fn used_in_one_hot_constraints(&self) -> &BTreeSet<OneHotConstraintID> {
        &self.used_in_one_hot_constraints
    }

    pub fn used_in_sos1_constraints(&self) -> &BTreeSet<Sos1ConstraintID> {
        &self.used_in_sos1_constraints
    }

    pub fn used_in_named_functions(&self) -> &BTreeSet<NamedFunctionID> {
        &self.used_in_named_functions
    }

    pub fn defines_dependent_variable(&self) -> bool {
        self.defines_dependent_variable
    }

    pub fn used_in_dependency_rhs_of(&self) -> &VariableIDSet {
        &self.used_in_dependency_rhs_of
    }

    pub fn is_used_by_solver(&self) -> bool {
        self.used_in_objective
            || !self.used_in_regular_constraints.is_empty()
            || !self.used_in_indicator_constraints.is_empty()
            || !self.used_in_one_hot_constraints.is_empty()
            || !self.used_in_sos1_constraints.is_empty()
    }
}

/// Self-contained reverse-usage snapshot for decision variables.
///
/// The source of truth remains [`Instance`]. This type is rebuilt from an
/// immutable instance reference and does not mutate or validate the instance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DecisionVariableUsageCore {
    by_variable: BTreeMap<VariableID, VariableUsage>,
}

impl DecisionVariableUsageCore {
    pub fn from_instance(instance: &Instance) -> Self {
        let mut by_variable: BTreeMap<VariableID, VariableUsage> = instance
            .decision_variables()
            .iter()
            .map(|(id, dv)| (*id, VariableUsage::new(dv)))
            .collect();

        for id in instance.objective().required_ids() {
            usage_entry_mut(&mut by_variable, id).used_in_objective = true;
        }

        for (constraint_id, constraint) in instance.constraints() {
            for id in constraint.function().required_ids() {
                usage_entry_mut(&mut by_variable, id)
                    .used_in_regular_constraints
                    .insert(*constraint_id);
            }
        }

        for (constraint_id, constraint) in instance.indicator_constraints() {
            for id in constraint.required_ids() {
                usage_entry_mut(&mut by_variable, id)
                    .used_in_indicator_constraints
                    .insert(*constraint_id);
            }
        }

        for (constraint_id, constraint) in instance.one_hot_constraints() {
            for id in constraint.required_ids() {
                usage_entry_mut(&mut by_variable, id)
                    .used_in_one_hot_constraints
                    .insert(*constraint_id);
            }
        }

        for (constraint_id, constraint) in instance.sos1_constraints() {
            for id in constraint.required_ids() {
                usage_entry_mut(&mut by_variable, id)
                    .used_in_sos1_constraints
                    .insert(*constraint_id);
            }
        }

        for (dependent_id, function) in instance.decision_variable_dependency.iter() {
            usage_entry_mut(&mut by_variable, *dependent_id).defines_dependent_variable = true;
            for required_id in function.required_ids() {
                usage_entry_mut(&mut by_variable, required_id)
                    .used_in_dependency_rhs_of
                    .insert(*dependent_id);
            }
        }

        for (named_function_id, named_function) in &instance.named_functions {
            for id in named_function.required_ids() {
                usage_entry_mut(&mut by_variable, id)
                    .used_in_named_functions
                    .insert(*named_function_id);
            }
        }

        for usage in by_variable.values_mut() {
            usage.refresh_role();
        }

        Self { by_variable }
    }

    pub fn by_variable(&self) -> &BTreeMap<VariableID, VariableUsage> {
        &self.by_variable
    }

    pub fn get(&self, id: VariableID) -> Option<&VariableUsage> {
        self.by_variable.get(&id)
    }

    pub fn role(&self, id: VariableID) -> Option<DecisionVariableRole> {
        self.get(id).map(VariableUsage::role)
    }

    pub fn all(&self) -> VariableIDSet {
        self.by_variable.keys().copied().collect()
    }

    pub fn used(&self) -> VariableIDSet {
        self.by_variable
            .iter()
            .filter_map(|(id, usage)| usage.is_used_by_solver().then_some(*id))
            .collect()
    }

    pub fn used_in_objective(&self) -> VariableIDSet {
        self.by_variable
            .iter()
            .filter_map(|(id, usage)| usage.used_in_objective.then_some(*id))
            .collect()
    }

    /// Variables used in active regular constraints, keyed by constraint ID.
    pub fn used_in_constraints(&self) -> BTreeMap<ConstraintID, VariableIDSet> {
        invert_usage_map(&self.by_variable, |usage| {
            &usage.used_in_regular_constraints
        })
    }

    /// Variables used in active indicator constraints, keyed by constraint ID.
    pub fn used_in_indicator_constraints(&self) -> BTreeMap<IndicatorConstraintID, VariableIDSet> {
        invert_usage_map(&self.by_variable, |usage| {
            &usage.used_in_indicator_constraints
        })
    }

    /// Variables used in active one-hot constraints, keyed by constraint ID.
    pub fn used_in_one_hot_constraints(&self) -> BTreeMap<OneHotConstraintID, VariableIDSet> {
        invert_usage_map(&self.by_variable, |usage| {
            &usage.used_in_one_hot_constraints
        })
    }

    /// Variables used in active SOS1 constraints, keyed by constraint ID.
    pub fn used_in_sos1_constraints(&self) -> BTreeMap<Sos1ConstraintID, VariableIDSet> {
        invert_usage_map(&self.by_variable, |usage| &usage.used_in_sos1_constraints)
    }

    /// Variables used in named functions, keyed by named-function ID.
    ///
    /// This is diagnostic usage and does not contribute to
    /// [`DecisionVariableRole::Used`].
    pub fn used_in_named_functions(&self) -> BTreeMap<NamedFunctionID, VariableIDSet> {
        invert_usage_map(&self.by_variable, |usage| &usage.used_in_named_functions)
    }

    pub fn fixed(&self) -> BTreeMap<VariableID, f64> {
        self.by_variable
            .iter()
            .filter_map(|(id, usage)| match usage.role {
                DecisionVariableRole::Fixed => usage.substituted_value.map(|value| (*id, value)),
                _ => None,
            })
            .collect()
    }

    pub fn dependent(&self) -> VariableIDSet {
        self.by_variable
            .iter()
            .filter_map(|(id, usage)| {
                (usage.role == DecisionVariableRole::Dependent).then_some(*id)
            })
            .collect()
    }

    pub fn irrelevant(&self) -> VariableIDSet {
        self.by_variable
            .iter()
            .filter_map(|(id, usage)| {
                (usage.role == DecisionVariableRole::Irrelevant).then_some(*id)
            })
            .collect()
    }
}

/// Reverse-usage view tied to the lifetime of an [`Instance`].
///
/// Rust callers can keep this view while borrowing the instance immutably. The
/// core snapshot is still exposed for bindings and for callers that need an
/// owned query object.
#[derive(Debug, Clone)]
pub struct DecisionVariableUsage<'a> {
    instance: &'a Instance,
    core: DecisionVariableUsageCore,
}

impl<'a> DecisionVariableUsage<'a> {
    fn new(instance: &'a Instance) -> Self {
        Self {
            instance,
            core: DecisionVariableUsageCore::from_instance(instance),
        }
    }

    pub fn instance(&self) -> &'a Instance {
        self.instance
    }

    pub fn core(&self) -> &DecisionVariableUsageCore {
        &self.core
    }

    pub fn into_core(self) -> DecisionVariableUsageCore {
        self.core
    }

    pub fn used_decision_variables(&self) -> BTreeMap<VariableID, &'a DecisionVariable> {
        let used_ids = self.core.used();
        self.instance
            .decision_variables()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, dv)| (*id, dv))
            .collect()
    }

    pub fn binary(&self) -> Bounds {
        self.by_kind(Kind::Binary, false)
    }

    pub fn integer(&self) -> Bounds {
        self.by_kind(Kind::Integer, false)
    }

    pub fn continuous(&self) -> Bounds {
        self.by_kind(Kind::Continuous, false)
    }

    pub fn semi_integer(&self) -> Bounds {
        self.by_kind(Kind::SemiInteger, false)
    }

    pub fn semi_continuous(&self) -> Bounds {
        self.by_kind(Kind::SemiContinuous, false)
    }

    pub fn used_binary(&self) -> Bounds {
        self.by_kind(Kind::Binary, true)
    }

    pub fn used_integer(&self) -> Bounds {
        self.by_kind(Kind::Integer, true)
    }

    pub fn used_continuous(&self) -> Bounds {
        self.by_kind(Kind::Continuous, true)
    }

    pub fn used_semi_integer(&self) -> Bounds {
        self.by_kind(Kind::SemiInteger, true)
    }

    pub fn used_semi_continuous(&self) -> Bounds {
        self.by_kind(Kind::SemiContinuous, true)
    }

    fn by_kind(&self, kind: Kind, used_only: bool) -> Bounds {
        let used = used_only.then(|| self.core.used());
        self.instance
            .decision_variables()
            .iter()
            .filter_map(|(id, dv)| {
                (dv.kind() == kind && used.as_ref().map_or(true, |used| used.contains(id)))
                    .then_some((*id, dv.bound()))
            })
            .collect()
    }
}

impl std::ops::Deref for DecisionVariableUsage<'_> {
    type Target = DecisionVariableUsageCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl Instance {
    pub fn binary_ids(&self) -> VariableIDSet {
        self.decision_variables
            .iter()
            .filter_map(|(id, dv)| {
                if dv.kind() == Kind::Binary {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn used_decision_variable_ids(&self) -> VariableIDSet {
        let mut used = self.objective.required_ids();
        used.extend(self.constraint_collection.required_ids());
        used.extend(self.indicator_constraint_collection.required_ids());
        used.extend(self.one_hot_constraint_collection.required_ids());
        used.extend(self.sos1_constraint_collection.required_ids());
        // Note: named_functions are intentionally excluded from the "used" set.
        // They are auxiliary quantities that can reference fixed/dependent variables.
        used
    }

    pub fn used_decision_variables(&self) -> BTreeMap<VariableID, &DecisionVariable> {
        let used_ids = self.used_decision_variable_ids();
        self.decision_variables
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, dv)| (*id, dv))
            .collect()
    }

    pub fn decision_variable_usage(&self) -> DecisionVariableUsage<'_> {
        DecisionVariableUsage::new(self)
    }

    pub fn decision_variable_usage_core(&self) -> DecisionVariableUsageCore {
        DecisionVariableUsageCore::from_instance(self)
    }
}

impl std::fmt::Display for DecisionVariableUsageCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_usage_display(f, self, None)
    }
}

impl std::fmt::Display for DecisionVariableUsage<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_usage_display(f, &self.core, Some(self))
    }
}

fn write_usage_display(
    f: &mut std::fmt::Formatter<'_>,
    core: &DecisionVariableUsageCore,
    usage_view: Option<&DecisionVariableUsage<'_>>,
) -> std::fmt::Result {
    writeln!(f, "DecisionVariableUsage {{")?;
    writeln!(f, "  Total Variables: {}", core.by_variable.len())?;
    writeln!(f)?;

    if let Some(usage) = usage_view {
        writeln!(f, "  Kind-based Partitioning:")?;
        writeln!(
            f,
            "    Binary: {}, Integer: {}, Continuous: {}, Semi-Integer: {}, Semi-Continuous: {}",
            usage.binary().len(),
            usage.integer().len(),
            usage.continuous().len(),
            usage.semi_integer().len(),
            usage.semi_continuous().len()
        )?;
        writeln!(f)?;
    }

    let used = core.used();
    let fixed = core.fixed();
    let dependent = core.dependent();
    let irrelevant = core.irrelevant();
    writeln!(f, "  Role Summary:")?;
    writeln!(
        f,
        "    Used: {}, Fixed: {}, Dependent: {}, Irrelevant: {}",
        used.len(),
        fixed.len(),
        dependent.len(),
        irrelevant.len()
    )?;

    if !core.used_in_objective().is_empty() {
        writeln!(
            f,
            "\n  Used in Objective ({}):",
            core.used_in_objective().len()
        )?;
        write_variable_list(f, core.used_in_objective().iter())?;
    }

    write_constraint_usage(
        f,
        "Used in Regular Constraints",
        &core.used_in_constraints(),
    )?;
    write_constraint_usage(
        f,
        "Used in Indicator Constraints",
        &core.used_in_indicator_constraints(),
    )?;
    write_constraint_usage(
        f,
        "Used in One-Hot Constraints",
        &core.used_in_one_hot_constraints(),
    )?;
    write_constraint_usage(
        f,
        "Used in SOS1 Constraints",
        &core.used_in_sos1_constraints(),
    )?;
    write_constraint_usage(
        f,
        "Used in Named Functions (diagnostic)",
        &core.used_in_named_functions(),
    )?;

    if !fixed.is_empty() {
        writeln!(f, "\n  Fixed Variables ({}):", fixed.len())?;
        for (id, value) in &fixed {
            writeln!(f, "    x{} = {}", id.into_inner(), value)?;
        }
    }

    if !dependent.is_empty() {
        writeln!(f, "\n  Dependent Variables ({}):", dependent.len())?;
        for id in &dependent {
            if let Some(usage) = usage_view {
                let dv = usage
                    .instance
                    .decision_variables()
                    .get(id)
                    .expect("ID comes from by_variable");
                writeln!(
                    f,
                    "    x{} ({:?}, {})",
                    id.into_inner(),
                    dv.kind(),
                    dv.bound()
                )?;
            } else {
                writeln!(f, "    x{}", id.into_inner())?;
            }
        }
    }

    let dependency_rhs: BTreeMap<VariableID, VariableIDSet> = core
        .by_variable
        .iter()
        .filter_map(|(id, usage)| {
            (!usage.used_in_dependency_rhs_of.is_empty())
                .then_some((*id, usage.used_in_dependency_rhs_of.clone()))
        })
        .collect();
    if !dependency_rhs.is_empty() {
        writeln!(
            f,
            "\n  Used in Dependent Variable Assignments ({} variables):",
            dependency_rhs.len()
        )?;
        for (id, dependent_ids) in &dependency_rhs {
            write!(f, "    x{} -> ", id.into_inner())?;
            write_variable_list_inline(f, dependent_ids.iter())?;
            writeln!(f)?;
        }
    }

    if !irrelevant.is_empty() {
        writeln!(f, "\n  Irrelevant Variables ({}):", irrelevant.len())?;
        for id in &irrelevant {
            if let Some(usage) = usage_view {
                let dv = usage
                    .instance
                    .decision_variables()
                    .get(id)
                    .expect("ID comes from by_variable");
                let default_value = dv.bound().nearest_to_zero();
                writeln!(
                    f,
                    "    x{} ({:?}, {}): will be set to {}",
                    id.into_inner(),
                    dv.kind(),
                    dv.bound(),
                    default_value
                )?;
            } else {
                writeln!(f, "    x{}", id.into_inner())?;
            }
        }
    }

    write!(f, "}}")
}

fn usage_entry_mut(
    by_variable: &mut BTreeMap<VariableID, VariableUsage>,
    id: VariableID,
) -> &mut VariableUsage {
    by_variable.get_mut(&id).unwrap_or_else(|| {
        panic!("Instance invariant violated: variable {id:?} is referenced but not registered")
    })
}

fn invert_usage_map<ID: Copy + Ord>(
    by_variable: &BTreeMap<VariableID, VariableUsage>,
    select: impl Fn(&VariableUsage) -> &BTreeSet<ID>,
) -> BTreeMap<ID, VariableIDSet> {
    let mut by_owner: BTreeMap<ID, VariableIDSet> = BTreeMap::new();
    for (variable_id, usage) in by_variable {
        for owner_id in select(usage) {
            by_owner.entry(*owner_id).or_default().insert(*variable_id);
        }
    }
    by_owner
}

fn write_constraint_usage<ID: std::fmt::Display>(
    f: &mut std::fmt::Formatter<'_>,
    title: &str,
    usage: &BTreeMap<ID, VariableIDSet>,
) -> std::fmt::Result {
    if usage.is_empty() {
        return Ok(());
    }
    writeln!(f, "\n  {title} ({}):", usage.len())?;
    for (owner_id, variable_ids) in usage {
        write!(f, "    {owner_id}: ")?;
        write_variable_list_inline(f, variable_ids.iter())?;
        writeln!(f)?;
    }
    Ok(())
}

fn write_variable_list<'a>(
    f: &mut std::fmt::Formatter<'_>,
    variable_ids: impl Iterator<Item = &'a VariableID>,
) -> std::fmt::Result {
    write!(f, "    ")?;
    write_variable_list_inline(f, variable_ids)?;
    writeln!(f)
}

fn write_variable_list_inline<'a>(
    f: &mut std::fmt::Formatter<'_>,
    variable_ids: impl Iterator<Item = &'a VariableID>,
) -> std::fmt::Result {
    let vars: Vec<String> = variable_ids
        .map(|id| format!("x{}", id.into_inner()))
        .collect();
    write!(f, "{}", vars.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assign, coeff, linear, v1::State, Constraint, ConstraintID, Evaluate, Sense, Substitute,
        VariableID,
    };
    use maplit::hashmap;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_decision_variable_usage_display() {
        // Create instance with 5 binary variables
        let mut decision_variables = BTreeMap::new();
        for i in 0..5 {
            decision_variables.insert(
                VariableID::from(i),
                crate::DecisionVariable::binary(VariableID::from(i)),
            );
        }

        // Objective: x0 + x1 + x2
        let objective = crate::Function::from(linear!(0) + linear!(1) + linear!(2));

        // Constraints:
        // 0: x1 + x2 == 1
        // 1: x3 == x0 + x1  (this will make x3 dependent after substitution)
        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(0),
            Constraint::equal_to_zero((linear!(1) + linear!(2) + coeff!(-1.0)).into()),
        );
        constraints.insert(
            ConstraintID::from(1),
            Constraint::equal_to_zero(
                (linear!(3) + coeff!(-1.0) * linear!(0) + coeff!(-1.0) * linear!(1)).into(),
            ),
        );
        let mut instance =
            Instance::new(Sense::Maximize, objective, decision_variables, constraints).unwrap();

        // Apply partial_evaluate to fix x0 = 1
        let state = State {
            entries: hashmap! { 0 => 1.0 },
        };
        instance
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // Apply substitute_acyclic to create dependent variables
        // x3 <- x0 + x1, but x0 is already fixed to 1, so x3 <- 1 + x1
        let substitutions = assign! {
            3 <- linear!(0) + linear!(1)
        };
        let instance = instance.substitute_acyclic(&substitutions).unwrap();

        let usage = instance.decision_variable_usage();
        insta::assert_snapshot!(usage);
    }

    /// Test that dependent variables are evaluated in topological order.
    ///
    /// This test creates a chain of dependent variables where:
    /// - x_10 = x_1 + x_2 (depends on independent variables)
    /// - x_5 = x_10 + 1 (depends on another dependent variable x_10)
    ///
    /// In BTreeMap order (by VariableID), x_5 would be evaluated before x_10,
    /// which would fail because x_10 is not yet in the state.
    /// With topological sort, x_10 is evaluated first, then x_5.
    #[test]
    fn test_populate_dependent_variables_topological_order() {
        use crate::{assign, coeff, linear, DecisionVariable, Sense};
        use maplit::btreemap;
        use std::collections::HashMap;

        // Create decision variables:
        // x_1, x_2: independent variables (used in objective)
        // x_5: dependent on x_10 (x_5 = x_10 + 1)
        // x_10: dependent on x_1, x_2 (x_10 = x_1 + x_2)
        //
        // Note: x_5 < x_10 in BTreeMap order, but x_10 must be evaluated first
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
            VariableID::from(5) => DecisionVariable::continuous(VariableID::from(5)),
            VariableID::from(10) => DecisionVariable::continuous(VariableID::from(10)),
        };

        // x_10 = x_1 + x_2
        // x_5 = x_10 + 1
        let dependency = assign! {
            10 <- linear!(1) + linear!(2),
            5 <- linear!(10) + coeff!(1.0)
        };

        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(crate::Function::from(linear!(1) + linear!(2))) // objective uses x_1 and x_2
            .decision_variables(decision_variables)
            .constraints(btreemap! {})
            .decision_variable_dependency(dependency)
            .build()
            .unwrap();

        // Verify x_5 and x_10 are both dependent
        let usage = instance.decision_variable_usage();
        assert!(usage.dependent().contains(&VariableID::from(5)));
        assert!(usage.dependent().contains(&VariableID::from(10)));

        // State with only independent variables
        let state = crate::v1::State::from(HashMap::from([(1, 2.0), (2, 3.0)]));

        // This should succeed with topological sort:
        // 1. Evaluate x_10 = x_1 + x_2 = 2.0 + 3.0 = 5.0
        // 2. Evaluate x_5 = x_10 + 1 = 5.0 + 1.0 = 6.0
        let populated = instance
            .populate_state(state, crate::ATol::default())
            .unwrap();

        assert_eq!(populated.entries.get(&1), Some(&2.0));
        assert_eq!(populated.entries.get(&2), Some(&3.0));
        assert_eq!(populated.entries.get(&10), Some(&5.0)); // x_10 = 2 + 3
        assert_eq!(populated.entries.get(&5), Some(&6.0)); // x_5 = 5 + 1
    }

    #[test]
    fn test_named_function_usage_is_diagnostic() {
        use crate::{linear, DecisionVariable, NamedFunction, NamedFunctionID};
        use maplit::btreemap;

        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
        };
        let named_function = NamedFunction {
            id: NamedFunctionID::from(7),
            function: linear!(2).into(),
        };
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(linear!(1).into())
            .decision_variables(decision_variables)
            .constraints(btreemap! {})
            .named_functions(btreemap! {
                NamedFunctionID::from(7) => named_function,
            })
            .build()
            .unwrap();

        let usage = instance.decision_variable_usage();
        assert_eq!(
            usage.role(VariableID::from(1)),
            Some(DecisionVariableRole::Used)
        );
        assert_eq!(
            usage.role(VariableID::from(2)),
            Some(DecisionVariableRole::Irrelevant)
        );
        assert_eq!(
            usage
                .get(VariableID::from(2))
                .unwrap()
                .used_in_named_functions(),
            &BTreeSet::from([NamedFunctionID::from(7)])
        );
    }

    proptest! {
        // Binary, integer, continuous, semi_integer, and semi_continuous are disjoint
        // and their union is equal to all.
        #[test]
        fn test_kind_partition(instance in Instance::arbitrary()) {
            let usage = instance.decision_variable_usage();
            let all = usage.all();
            prop_assert_eq!(
                all.len(),
                usage.binary().len() + usage.integer().len() + usage.continuous().len()
                + usage.semi_integer().len() + usage.semi_continuous().len()
            );
            let mut by_kind_all: VariableIDSet = usage.binary().keys().cloned().collect();
            prop_assert_eq!(&by_kind_all, &instance.binary_ids());

            by_kind_all.extend(usage.integer().keys());
            by_kind_all.extend(usage.continuous().keys());
            by_kind_all.extend(usage.semi_integer().keys());
            by_kind_all.extend(usage.semi_continuous().keys());
            prop_assert_eq!(&by_kind_all, &all);
        }

        // Used, fixed, dependent, and irrelevant roles are disjoint, and their union is equal to all.
        #[test]
        fn test_role_partition(instance in Instance::arbitrary()) {
            let usage = instance.decision_variable_usage();
            let used = usage.used();
            let all = usage.all();
            let fixed = usage.fixed();
            let dependent = usage.dependent();
            let irrelevant = usage.irrelevant();
            prop_assert_eq!(&used, &instance.used_decision_variable_ids());
            prop_assert_eq!(
                all.len(),
                used.len() + fixed.len() + dependent.len() + irrelevant.len(),
                "all: {}, used: {}, fixed: {}, dependent: {}, irrelevant: {}",
                all.len(), used.len(), fixed.len(), dependent.len(), irrelevant.len()
            );
            let mut role_all = used.clone();
            role_all.extend(fixed.keys());
            role_all.extend(dependent);
            role_all.extend(irrelevant);
            prop_assert_eq!(&role_all, &all);
        }

        /// Test post-condition
        #[test]
        fn test_populate(
            (instance, state) in Instance::arbitrary()
                .prop_flat_map(move |instance| instance.arbitrary_state().prop_map(move |state| (instance.clone(), state)))
        ) {
            let populated = instance
                .populate_state(state.clone(), crate::ATol::default())
                .unwrap();
            let populated_ids: VariableIDSet = populated.entries.keys().map(|id| (*id).into()).collect();
            prop_assert_eq!(
                populated_ids,
                instance.decision_variables().keys().copied().collect()
            );
        }
    }
}
