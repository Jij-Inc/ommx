//! Own the ordinary formulations, stable IDs, and checked promotion requests.

use anyhow::{ensure, Result};
use ommx::{
    v1, ATol, Bound, Constraint, ConstraintContext, DecisionVariable, Function, Instance, Kind,
    Linear, LinearMonomial, ModelingLabel, Sense, Sos1BigMPromotionRequest, Sos1BigMSelectorClaim,
    VariableID, VariableLabelStore,
};
use std::collections::BTreeMap;

const CARDINALITY_ID: u64 = 23;

fn linear(terms: impl IntoIterator<Item = (u64, f64)>, constant: f64) -> Result<Function> {
    let mut function = Linear::default();
    for (id, coefficient) in terms {
        if coefficient != 0.0 {
            function.add_term(LinearMonomial::Variable(id.into()), coefficient.try_into()?)?;
        }
    }
    if constant != 0.0 {
        function.add_term(LinearMonomial::Constant, constant.try_into()?)?;
    }
    Ok(function.into())
}

fn objective(costs: &[f64]) -> Result<Function> {
    ensure!(!costs.is_empty(), "costs must not be empty");
    linear(
        costs.iter().enumerate().map(|(i, &cost)| (i as u64, cost)),
        0.0,
    )
}

fn labels(count: usize) -> VariableLabelStore {
    let mut labels = VariableLabelStore::new();
    for i in 0..count {
        labels.set_name((i as u64).into(), "x");
        labels.set_subscripts((i as u64).into(), [i as i64]);
    }
    labels
}

fn label_cardinality(instance: &mut Instance, name: &str) -> Result<()> {
    instance.set_constraint_context(
        CARDINALITY_ID.into(),
        ConstraintContext {
            label: ModelingLabel {
                name: Some(name.to_owned()),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    Ok(())
}

pub fn one_hot(costs: &[f64]) -> Result<v1::Instance> {
    let mut instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(objective(costs)?)
        .decision_variables(
            (0..costs.len())
                .map(|i| ((i as u64).into(), DecisionVariable::binary()))
                .collect(),
        )
        .variable_labels(labels(costs.len()))
        .constraints(BTreeMap::from([(
            CARDINALITY_ID.into(),
            Constraint::equal_to_zero(linear((0..costs.len()).map(|i| (i as u64, 1.0)), -1.0)?),
        )]))
        .build()?;
    label_cardinality(&mut instance, "one_hot")?;
    let hints = instance
        .plan_promote_one_hot(&[CARDINALITY_ID.into()].into_iter().collect())
        .into_v1_hints()?;
    instance.into_v1_with_hints(hints)
}

pub fn sos1(costs: &[f64], bounds: &[(f64, f64)]) -> Result<v1::Instance> {
    ensure!(
        costs.len() == bounds.len(),
        "costs and bounds must have the same length"
    );
    let objective = objective(costs)?;
    let mut variables = BTreeMap::new();
    let mut variable_labels = labels(costs.len());
    let mut members = Vec::new();
    for (i, &(lower, upper)) in bounds.iter().enumerate() {
        ensure!(
            lower.is_finite() && upper.is_finite(),
            "SOS1 bounds must be finite"
        );
        let id = i as u64;
        variables.insert(
            id.into(),
            DecisionVariable::new(Kind::Continuous, Bound::new(lower, upper)?, ATol::default())?,
        );
        if lower != 0.0 || upper != 0.0 {
            members.push((id, lower, upper));
        }
    }

    let mut constraints = BTreeMap::new();
    let mut claims = BTreeMap::new();
    if members.len() >= 2 {
        for &(id, lower, upper) in &members {
            let selector = costs.len() as u64 + id;
            variables.insert(selector.into(), DecisionVariable::binary());
            variable_labels.set_name(selector.into(), "selector");
            variable_labels.set_subscripts(selector.into(), [id as i64]);
            let upper_link = if upper > 0.0 {
                let row = (100 + 2 * id).into();
                constraints.insert(
                    row,
                    Constraint::less_than_or_equal_to_zero(linear(
                        [(id, 1.0), (selector, -upper)],
                        0.0,
                    )?),
                );
                Some(row)
            } else {
                None
            };
            let lower_link = if lower < 0.0 {
                let row = (101 + 2 * id).into();
                constraints.insert(
                    row,
                    Constraint::less_than_or_equal_to_zero(linear(
                        [(id, -1.0), (selector, lower)],
                        0.0,
                    )?),
                );
                Some(row)
            } else {
                None
            };
            claims.insert(
                VariableID::from(id),
                Sos1BigMSelectorClaim::Fresh {
                    selector: selector.into(),
                    upper_link,
                    lower_link,
                },
            );
        }
        constraints.insert(
            CARDINALITY_ID.into(),
            Constraint::less_than_or_equal_to_zero(linear(
                members
                    .iter()
                    .map(|&(id, _, _)| (costs.len() as u64 + id, 1.0)),
                -1.0,
            )?),
        );
    }

    let mut instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(objective)
        .decision_variables(variables)
        .variable_labels(variable_labels)
        .constraints(constraints)
        .build()?;
    let mut request = Sos1BigMPromotionRequest::new();
    if !claims.is_empty() {
        label_cardinality(&mut instance, "sos1_cardinality")?;
        request.insert(CARDINALITY_ID.into(), claims);
    }
    let hints = instance
        .plan_promote_sos1_big_m(&request)
        .apply_bound_tightening_and_convert_to_v1_hints()?;
    instance.into_v1_with_hints(hints)
}

pub fn absolute_objective(costs: &[f64]) -> Result<v1::Instance> {
    let instance = Instance::builder()
        .sense(Sense::Minimize)
        .objective(objective(costs)?.abs())
        .decision_variables(
            (0..costs.len())
                .map(|i| ((i as u64).into(), DecisionVariable::continuous()))
                .collect(),
        )
        .variable_labels(labels(costs.len()))
        .constraints(BTreeMap::new())
        .build()?;
    instance.try_into()
}
