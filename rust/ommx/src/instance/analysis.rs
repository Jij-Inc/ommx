use super::Instance;
use crate::{
    Bounds, ConstraintID, DecisionVariable, Evaluate, IndicatorConstraintID, Kind,
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
/// Named functions and decision-variable dependency RHS expressions are
/// intentionally not solver-used.
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

/// Reverse-usage entry for one used decision variable.
///
/// A variable has an entry here only when it appears in the objective or an
/// active constraint family that contributes to solver input. Fixed,
/// dependent, and irrelevant variables do not have usage entries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecisionVariableUsageEntry {
    used_in_objective: bool,
    used_in_regular_constraints: BTreeSet<ConstraintID>,
    used_in_indicator_constraints: BTreeSet<IndicatorConstraintID>,
    used_in_one_hot_constraints: BTreeSet<OneHotConstraintID>,
    used_in_sos1_constraints: BTreeSet<Sos1ConstraintID>,
}

impl DecisionVariableUsageEntry {
    fn new() -> Self {
        Self {
            used_in_objective: false,
            used_in_regular_constraints: BTreeSet::new(),
            used_in_indicator_constraints: BTreeSet::new(),
            used_in_one_hot_constraints: BTreeSet::new(),
            used_in_sos1_constraints: BTreeSet::new(),
        }
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

    pub fn is_used_by_solver(&self) -> bool {
        self.used_in_objective
            || !self.used_in_regular_constraints.is_empty()
            || !self.used_in_indicator_constraints.is_empty()
            || !self.used_in_one_hot_constraints.is_empty()
            || !self.used_in_sos1_constraints.is_empty()
    }
}

/// Reverse-usage view tied to the lifetime of an [`Instance`].
///
/// The usage index contains only variables that are used by solver input. The
/// borrowed [`Instance`] remains the source of truth for decision-variable
/// classification such as fixed, dependent, and irrelevant.
#[derive(Debug, Clone)]
pub struct DecisionVariableUsage<'a> {
    instance: &'a Instance,
    by_used_variable: BTreeMap<VariableID, DecisionVariableUsageEntry>,
}

impl<'a> DecisionVariableUsage<'a> {
    fn new(instance: &'a Instance) -> Self {
        let mut by_used_variable: BTreeMap<VariableID, DecisionVariableUsageEntry> =
            BTreeMap::new();

        for id in instance.objective().required_ids() {
            usage_entry_mut(&mut by_used_variable, id).used_in_objective = true;
        }

        for (constraint_id, constraint) in instance.constraints() {
            for id in constraint.function().required_ids() {
                usage_entry_mut(&mut by_used_variable, id)
                    .used_in_regular_constraints
                    .insert(*constraint_id);
            }
        }

        for (constraint_id, constraint) in instance.indicator_constraints() {
            for id in constraint.required_ids() {
                usage_entry_mut(&mut by_used_variable, id)
                    .used_in_indicator_constraints
                    .insert(*constraint_id);
            }
        }

        for (constraint_id, constraint) in instance.one_hot_constraints() {
            for id in constraint.required_ids() {
                usage_entry_mut(&mut by_used_variable, id)
                    .used_in_one_hot_constraints
                    .insert(*constraint_id);
            }
        }

        for (constraint_id, constraint) in instance.sos1_constraints() {
            for id in constraint.required_ids() {
                usage_entry_mut(&mut by_used_variable, id)
                    .used_in_sos1_constraints
                    .insert(*constraint_id);
            }
        }

        Self {
            instance,
            by_used_variable,
        }
    }

    pub fn instance(&self) -> &'a Instance {
        self.instance
    }

    pub fn by_used_variable(&self) -> &BTreeMap<VariableID, DecisionVariableUsageEntry> {
        &self.by_used_variable
    }

    pub fn get(&self, id: VariableID) -> Option<&DecisionVariableUsageEntry> {
        self.by_used_variable.get(&id)
    }

    pub fn role(&self, id: VariableID) -> Option<DecisionVariableRole> {
        self.instance.decision_variable_role(id)
    }

    pub fn roles(&self) -> BTreeMap<VariableID, DecisionVariableRole> {
        self.instance.decision_variable_roles()
    }

    pub fn used(&self) -> VariableIDSet {
        self.by_used_variable.keys().copied().collect()
    }

    pub fn used_in_objective(&self) -> VariableIDSet {
        self.by_used_variable
            .iter()
            .filter_map(|(id, usage)| usage.used_in_objective.then_some(*id))
            .collect()
    }

    /// Variables used in active regular constraints, keyed by constraint ID.
    pub fn used_in_constraints(&self) -> BTreeMap<ConstraintID, VariableIDSet> {
        invert_usage_map(&self.by_used_variable, |usage| {
            &usage.used_in_regular_constraints
        })
    }

    /// Variables used in active indicator constraints, keyed by constraint ID.
    pub fn used_in_indicator_constraints(&self) -> BTreeMap<IndicatorConstraintID, VariableIDSet> {
        invert_usage_map(&self.by_used_variable, |usage| {
            &usage.used_in_indicator_constraints
        })
    }

    /// Variables used in active one-hot constraints, keyed by constraint ID.
    pub fn used_in_one_hot_constraints(&self) -> BTreeMap<OneHotConstraintID, VariableIDSet> {
        invert_usage_map(&self.by_used_variable, |usage| {
            &usage.used_in_one_hot_constraints
        })
    }

    /// Variables used in active SOS1 constraints, keyed by constraint ID.
    pub fn used_in_sos1_constraints(&self) -> BTreeMap<Sos1ConstraintID, VariableIDSet> {
        invert_usage_map(&self.by_used_variable, |usage| {
            &usage.used_in_sos1_constraints
        })
    }

    pub fn fixed(&self) -> BTreeMap<VariableID, f64> {
        self.instance.fixed_decision_variables()
    }

    pub fn dependent(&self) -> VariableIDSet {
        self.instance.dependent_decision_variable_ids()
    }

    pub fn irrelevant(&self) -> VariableIDSet {
        self.instance.irrelevant_decision_variable_ids()
    }

    pub fn used_decision_variables(&self) -> BTreeMap<VariableID, &'a DecisionVariable> {
        self.by_used_variable
            .keys()
            .map(|id| {
                let dv = self
                    .instance
                    .decision_variables()
                    .get(id)
                    .expect("used variable ID must be defined in decision_variables");
                (*id, dv)
            })
            .collect()
    }

    pub fn used_binary(&self) -> Bounds {
        self.used_by_kind(Kind::Binary)
    }

    pub fn used_integer(&self) -> Bounds {
        self.used_by_kind(Kind::Integer)
    }

    pub fn used_continuous(&self) -> Bounds {
        self.used_by_kind(Kind::Continuous)
    }

    pub fn used_semi_integer(&self) -> Bounds {
        self.used_by_kind(Kind::SemiInteger)
    }

    pub fn used_semi_continuous(&self) -> Bounds {
        self.used_by_kind(Kind::SemiContinuous)
    }

    fn used_by_kind(&self, kind: Kind) -> Bounds {
        self.by_used_variable
            .keys()
            .filter_map(|id| {
                let dv = self
                    .instance
                    .decision_variables()
                    .get(id)
                    .expect("used variable ID must be defined in decision_variables");
                (dv.kind() == kind).then_some((*id, dv.bound()))
            })
            .collect()
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

    pub fn decision_variable_role(&self, id: VariableID) -> Option<DecisionVariableRole> {
        let used = self.used_decision_variable_ids();
        self.decision_variables
            .get(&id)
            .map(|dv| self.decision_variable_role_with_used(id, dv, &used))
    }

    pub fn decision_variable_roles(&self) -> BTreeMap<VariableID, DecisionVariableRole> {
        let used = self.used_decision_variable_ids();
        self.decision_variables
            .iter()
            .map(|(id, dv)| (*id, self.decision_variable_role_with_used(*id, dv, &used)))
            .collect()
    }

    pub fn fixed_decision_variables(&self) -> BTreeMap<VariableID, f64> {
        self.decision_variable_roles()
            .into_iter()
            .filter_map(|(id, role)| {
                (role == DecisionVariableRole::Fixed).then(|| {
                    let value = self
                        .decision_variables
                        .get(&id)
                        .and_then(DecisionVariable::substituted_value)
                        .expect("fixed role requires substituted_value");
                    (id, value)
                })
            })
            .collect()
    }

    pub fn dependent_decision_variable_ids(&self) -> VariableIDSet {
        self.decision_variable_roles()
            .into_iter()
            .filter_map(|(id, role)| (role == DecisionVariableRole::Dependent).then_some(id))
            .collect()
    }

    pub fn irrelevant_decision_variable_ids(&self) -> VariableIDSet {
        self.decision_variable_roles()
            .into_iter()
            .filter_map(|(id, role)| (role == DecisionVariableRole::Irrelevant).then_some(id))
            .collect()
    }

    fn decision_variable_role_with_used(
        &self,
        id: VariableID,
        dv: &DecisionVariable,
        used: &VariableIDSet,
    ) -> DecisionVariableRole {
        if used.contains(&id) {
            DecisionVariableRole::Used
        } else if dv.substituted_value().is_some() {
            DecisionVariableRole::Fixed
        } else if self.decision_variable_dependency.get(&id).is_some() {
            DecisionVariableRole::Dependent
        } else {
            DecisionVariableRole::Irrelevant
        }
    }

    pub fn decision_variable_usage(&self) -> DecisionVariableUsage<'_> {
        DecisionVariableUsage::new(self)
    }
}

impl std::fmt::Display for DecisionVariableUsage<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_usage_display(f, self)
    }
}

fn write_usage_display(
    f: &mut std::fmt::Formatter<'_>,
    usage: &DecisionVariableUsage<'_>,
) -> std::fmt::Result {
    writeln!(f, "DecisionVariableUsage {{")?;
    writeln!(f, "  Used Variables: {}", usage.by_used_variable.len())?;

    let used_in_objective = usage.used_in_objective();
    if !used_in_objective.is_empty() {
        writeln!(f, "\n  Used in Objective ({}):", used_in_objective.len())?;
        write_variable_list(f, used_in_objective.iter())?;
    }

    write_constraint_usage(
        f,
        "Used in Regular Constraints",
        &usage.used_in_constraints(),
    )?;
    write_constraint_usage(
        f,
        "Used in Indicator Constraints",
        &usage.used_in_indicator_constraints(),
    )?;
    write_constraint_usage(
        f,
        "Used in One-Hot Constraints",
        &usage.used_in_one_hot_constraints(),
    )?;
    write_constraint_usage(
        f,
        "Used in SOS1 Constraints",
        &usage.used_in_sos1_constraints(),
    )?;

    write!(f, "}}")
}

fn usage_entry_mut(
    by_variable: &mut BTreeMap<VariableID, DecisionVariableUsageEntry>,
    id: VariableID,
) -> &mut DecisionVariableUsageEntry {
    by_variable
        .entry(id)
        .or_insert_with(DecisionVariableUsageEntry::new)
}

fn invert_usage_map<ID: Copy + Ord>(
    by_variable: &BTreeMap<VariableID, DecisionVariableUsageEntry>,
    select: impl Fn(&DecisionVariableUsageEntry) -> &BTreeSet<ID>,
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
        let objective =
            crate::Function::from(((linear!(0) + linear!(1)).unwrap() + linear!(2)).unwrap());

        // Constraints:
        // 0: x1 + x2 == 1
        // 1: x3 == x0 + x1  (this will make x3 dependent after substitution)
        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(0),
            Constraint::equal_to_zero(
                ((linear!(1) + linear!(2)).unwrap() + coeff!(-1.0))
                    .unwrap()
                    .into(),
            ),
        );
        constraints.insert(
            ConstraintID::from(1),
            Constraint::equal_to_zero(
                ((linear!(3) + (coeff!(-1.0) * linear!(0)).unwrap()).unwrap()
                    + (coeff!(-1.0) * linear!(1)).unwrap())
                .unwrap()
                .into(),
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
    fn test_named_function_does_not_create_usage() {
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
        assert!(usage.get(VariableID::from(2)).is_none());
    }

    proptest! {
        // Used binary, integer, continuous, semi_integer, and semi_continuous sets
        // are disjoint and their union is equal to all used variables.
        #[test]
        fn test_used_kind_partition(instance in Instance::arbitrary()) {
            let usage = instance.decision_variable_usage();
            let used = usage.used();
            prop_assert_eq!(
                used.len(),
                usage.used_binary().len() + usage.used_integer().len() + usage.used_continuous().len()
                + usage.used_semi_integer().len() + usage.used_semi_continuous().len()
            );
            let mut by_kind_all: VariableIDSet = usage.used_binary().keys().cloned().collect();

            by_kind_all.extend(usage.used_integer().keys());
            by_kind_all.extend(usage.used_continuous().keys());
            by_kind_all.extend(usage.used_semi_integer().keys());
            by_kind_all.extend(usage.used_semi_continuous().keys());
            prop_assert_eq!(&by_kind_all, &used);
        }

        // Used, fixed, dependent, and irrelevant roles are disjoint, and their union is equal to all.
        #[test]
        fn test_role_partition(instance in Instance::arbitrary()) {
            let usage = instance.decision_variable_usage();
            let used = usage.used();
            let all: VariableIDSet = instance.decision_variables().keys().copied().collect();
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
