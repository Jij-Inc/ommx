use crate::{VariableID, VariableIDSet};
use std::collections::BTreeSet;

/// Invalid references supplied when constructing or modifying an
/// [`Instance`](super::Instance) or [`ParametricInstance`](super::ParametricInstance).
///
/// Each emitted variant contains every offending ID in the reference set being
/// checked, in ascending order. A builder may check several expressions in turn
/// and report the first failing set. Fix the indicated references and retry;
/// setters validate before changing the instance.
///
/// Function bodies in a parametric instance may reference declared parameters.
/// Structural positions (indicator controls and OneHot/SOS1 members) require
/// decision variables. Retained expressions such as named functions may refer
/// to fixed or dependent variables; active objectives and constraints may not.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VariableReferenceError {
    /// Declare these IDs in the instance or correct the expression's references.
    #[error("Undefined variable IDs are used: {ids:?}")]
    Undefined { ids: BTreeSet<VariableID> },
    /// Substitute these variables' dependency expressions before using them.
    #[error("Dependent variables cannot be used in objectives or constraints: {ids:?}")]
    Dependent { ids: BTreeSet<VariableID> },
    /// Substitute these variables' fixed values before using them.
    #[error("Fixed variables cannot be used in objectives or constraints: {ids:?}")]
    Fixed { ids: BTreeSet<VariableID> },
    /// Use decision variables instead of parameters in structural positions.
    #[error("Parameter IDs cannot occupy a structural variable position; use decision variables: {ids:?}")]
    ParameterInStructuralPosition { ids: BTreeSet<VariableID> },
}

impl VariableReferenceError {
    /// All offending IDs for the reported reason in the checked reference set.
    pub fn ids(&self) -> &BTreeSet<VariableID> {
        match self {
            Self::Undefined { ids }
            | Self::Dependent { ids }
            | Self::Fixed { ids }
            | Self::ParameterInStructuralPosition { ids } => ids,
        }
    }
}

pub fn validate_defined_ids(required: &VariableIDSet, known: &VariableIDSet) -> crate::Result<()> {
    let ids: BTreeSet<_> = required.difference(known).copied().collect();
    if !ids.is_empty() {
        return Err(VariableReferenceError::Undefined { ids }.into());
    }
    Ok(())
}

pub fn validate_active_ids(
    required: &VariableIDSet,
    dependent: &VariableIDSet,
    fixed: &VariableIDSet,
) -> crate::Result<()> {
    let ids: BTreeSet<_> = required.intersection(dependent).copied().collect();
    if !ids.is_empty() {
        return Err(VariableReferenceError::Dependent { ids }.into());
    }
    let ids: BTreeSet<_> = required.intersection(fixed).copied().collect();
    if !ids.is_empty() {
        return Err(VariableReferenceError::Fixed { ids }.into());
    }
    Ok(())
}

pub fn validate_structural_ids(
    required: &VariableIDSet,
    variables: &VariableIDSet,
    parameters: &VariableIDSet,
) -> crate::Result<()> {
    let known = variables.union(parameters).copied().collect();
    validate_defined_ids(required, &known)?;
    let ids: BTreeSet<_> = required.intersection(parameters).copied().collect();
    if !ids.is_empty() {
        return Err(VariableReferenceError::ParameterInStructuralPosition { ids }.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assign, coeff, linear, Constraint, Equality, Function, IndicatorConstraint,
        IndicatorTriggerValue, Instance, OneHotConstraint, ParameterTable, ParametricInstance,
        Sense, Sos1Constraint,
    };
    use std::collections::BTreeMap;

    fn variables() -> BTreeMap<VariableID, crate::DecisionVariable> {
        [0, 10, 20]
            .map(|id| (id.into(), crate::DecisionVariable::binary()))
            .into()
    }

    fn ids() -> BTreeSet<VariableID> {
        [10.into(), 20.into()].into()
    }

    fn function() -> Function {
        (linear!(10) + linear!(20)).unwrap().into()
    }

    fn indicator() -> IndicatorConstraint {
        IndicatorConstraint::new(
            0.into(),
            Equality::EqualToZero,
            function(),
            IndicatorTriggerValue::One,
        )
    }

    fn check(error: crate::Error, expected: &VariableReferenceError) {
        let signal = error.downcast_ref::<VariableReferenceError>().unwrap();
        assert_eq!(signal, expected);
        assert_eq!(signal.ids(), &ids());
    }

    #[test]
    fn instance_mutations_report_all_ids_and_leave_the_owner_unchanged() {
        for reason in 0..3 {
            let mut builder = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::Zero)
                .decision_variables(variables())
                .active_constraints(BTreeMap::new());
            let expected = match reason {
                0 => {
                    builder = builder.decision_variables(BTreeMap::from([(
                        0.into(),
                        crate::DecisionVariable::binary(),
                    )]));
                    VariableReferenceError::Undefined { ids: ids() }
                }
                1 => {
                    builder = builder.fixed_decision_variable_values(
                        [(10.into(), 0.0), (20.into(), 1.0)].into(),
                    );
                    VariableReferenceError::Fixed { ids: ids() }
                }
                _ => {
                    builder = builder.decision_variable_dependency(assign! {
                        10 <- Function::Zero,
                        20 <- coeff!(1.0)
                    });
                    VariableReferenceError::Dependent { ids: ids() }
                }
            };
            let original = builder.build().unwrap();
            for operation in 0..7 {
                let mut instance = original.clone();
                let constraint = Constraint::equal_to_zero(function());
                let error = match operation {
                    0 => instance.set_objective(function()).unwrap_err(),
                    1 => instance
                        .add_constraint(constraint, Default::default())
                        .unwrap_err(),
                    2 => instance
                        .insert_constraint(1.into(), constraint)
                        .unwrap_err(),
                    3 => instance
                        .insert_constraints(vec![
                            (1.into(), Constraint::equal_to_zero(Function::Zero)),
                            (2.into(), constraint),
                        ])
                        .unwrap_err(),
                    4 => instance
                        .add_indicator_constraint(indicator(), Default::default())
                        .unwrap_err(),
                    5 => instance
                        .add_one_hot_constraint(
                            OneHotConstraint::new(ids()).unwrap(),
                            Default::default(),
                        )
                        .unwrap_err(),
                    _ => instance
                        .add_sos1_constraint(
                            Sos1Constraint::new(ids()).unwrap(),
                            Default::default(),
                        )
                        .unwrap_err(),
                };
                check(error, &expected);
                assert_eq!(instance, original);
            }
        }
    }

    #[test]
    fn parametric_mutations_distinguish_parameters_and_invalid_references() {
        for reason in 0..4 {
            let mut builder = ParametricInstance::builder()
                .sense(Sense::Minimize)
                .objective(Function::Zero)
                .decision_variables(variables())
                .parameters(ParameterTable::default())
                .active_constraints(BTreeMap::new());
            let expected = match reason {
                0 | 3 => {
                    builder = builder.decision_variables(BTreeMap::from([(
                        0.into(),
                        crate::DecisionVariable::binary(),
                    )]));
                    if reason == 3 {
                        builder = builder.parameters(ParameterTable::from_ids(ids()));
                        VariableReferenceError::ParameterInStructuralPosition { ids: ids() }
                    } else {
                        VariableReferenceError::Undefined { ids: ids() }
                    }
                }
                1 => {
                    builder = builder.fixed_decision_variable_values(
                        [(10.into(), 0.0), (20.into(), 1.0)].into(),
                    );
                    VariableReferenceError::Fixed { ids: ids() }
                }
                _ => {
                    builder = builder.decision_variable_dependency(assign! {
                        10 <- Function::Zero,
                        20 <- coeff!(1.0)
                    });
                    VariableReferenceError::Dependent { ids: ids() }
                }
            };
            let original = builder.build().unwrap();
            for family in 0..4 {
                let mut instance = original.clone();
                let result = match family {
                    0 => instance
                        .add_constraint(Constraint::equal_to_zero(function()), Default::default())
                        .map(|_| ()),
                    1 => instance
                        .add_indicator_constraint(indicator(), Default::default())
                        .map(|_| ()),
                    2 => instance
                        .add_one_hot_constraint(
                            OneHotConstraint::new(ids()).unwrap(),
                            Default::default(),
                        )
                        .map(|_| ()),
                    _ => instance
                        .add_sos1_constraint(
                            Sos1Constraint::new(ids()).unwrap(),
                            Default::default(),
                        )
                        .map(|_| ()),
                };
                if reason == 3 && family < 2 {
                    result.unwrap(); // Declared parameters are valid in function bodies.
                } else {
                    check(result.unwrap_err(), &expected);
                    assert_eq!(instance, original);
                }
            }
        }
    }

    #[test]
    fn builders_and_named_functions_preserve_the_reference_signal() {
        let expected = VariableReferenceError::Undefined { ids: ids() };
        let builder = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .active_constraints(BTreeMap::new());
        check(
            builder.clone().objective(function()).build().unwrap_err(),
            &expected,
        );
        check(
            builder
                .clone()
                .active_constraints(BTreeMap::from([(
                    0.into(),
                    Constraint::equal_to_zero(function()),
                )]))
                .build()
                .unwrap_err(),
            &expected,
        );
        check(
            builder
                .clone()
                .named_functions(BTreeMap::from([(
                    0.into(),
                    crate::NamedFunction {
                        function: function(),
                    },
                )]))
                .build()
                .unwrap_err(),
            &expected,
        );
        let mut instance = builder.build().unwrap();
        let original = instance.clone();
        check(
            instance
                .new_named_function(function(), None, vec![], Default::default(), None)
                .unwrap_err(),
            &expected,
        );
        assert_eq!(instance, original);

        // A declared parameter must not be included among the unknown IDs.
        check(
            ParametricInstance::builder()
                .sense(Sense::Minimize)
                .objective((function() + Function::from(linear!(100))).unwrap())
                .decision_variables(BTreeMap::new())
                .parameters(ParameterTable::from_ids([100.into()].into()))
                .active_constraints(BTreeMap::new())
                .build()
                .unwrap_err(),
            &expected,
        );
    }
}
