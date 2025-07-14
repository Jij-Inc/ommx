use num::Zero;
use std::collections::{BTreeMap, HashMap, HashSet};

use super::{
    format::{CONSTR_PREFIX, OBJ_NAME, VAR_PREFIX},
    parser::{ColumnName, ObjSense, RowName},
    Mps,
};
use crate::{decision_variable::Kind as DecisionVariableKind, Coefficient};
use crate::{
    v1, Bound, Constraint, ConstraintHints, ConstraintID, DecisionVariable, Equality, Function,
    Instance, Sense, VariableID,
};

pub fn convert(mps: Mps) -> anyhow::Result<Instance> {
    let (decision_variables, name_id_map) = convert_dvars(&mps);
    let objective = convert_objective(&mps, &name_id_map)?;
    let constraints = convert_constraints(&mps, &name_id_map)?;
    let sense = convert_sense(mps.obj_sense);

    let mut instance = Instance::new(
        sense,
        objective,
        decision_variables,
        constraints,
        ConstraintHints::default(),
    )?;

    instance.description = convert_description(&mps);

    Ok(instance)
}

fn convert_description(mps: &Mps) -> Option<v1::instance::Description> {
    // currently only gets the name
    if mps.name.is_empty() {
        None
    } else {
        Some(v1::instance::Description {
            name: Some(mps.name.to_owned()),
            ..Default::default()
        })
    }
}

fn convert_dvars(
    mps: &Mps,
) -> (
    BTreeMap<VariableID, DecisionVariable>,
    HashMap<ColumnName, VariableID>,
) {
    let Mps {
        vars,
        u,
        l,
        integer,
        binary,
        real,
        ..
    } = mps;
    let mut dvars = BTreeMap::new();
    // Will be used to keep track of dvar ids throughout the conversion. might
    // not be strictly necessary if we make it so parser.rs and this file
    // guarantee a strict and consistent ordering, but it's less error-prone
    // this way
    let mut name_id_map = HashMap::with_capacity(u.len());

    // We want to be able to recover IDs if the var name is in the form
    // "OMMX_VAR_<number>", matching how our output formats names.
    //
    // NOTE considering the case where an OMMX-created MPS file was later
    // edited, there may be a mix of valid OMMX id names and invalid names.
    // Handling all edge cases would be pretty complex and potentially bad for
    // performance. For simplicity, we only apply ID recovery when ALL variables
    // match the naming pattern.
    if vars.iter().any(|name| !name.starts_with(VAR_PREFIX)) {
        // general case -- assign ids by order
        for (i, var_name) in vars.iter().enumerate() {
            let kind = get_dvar_kind(var_name, integer, binary, real);
            let bound = get_dvar_bound(var_name, l, u);
            // our ID ends up being dependent on the order of vars hashset. This is
            // unstable across executions -- we might want to consider an indexset
            // in the future
            let id = VariableID::from(i as u64);
            name_id_map.insert(var_name.clone(), id);
            let dvar = DecisionVariable::new(id, kind, bound, None, crate::ATol::default())
                .expect("Failed to create decision variable");
            // Set the name if available
            // Note: The current DecisionVariable API doesn't have with_name method
            dvars.insert(id, dvar);
        }
    } else {
        // recover IDs case
        for (id_value, var_name) in vars
            .iter()
            .filter_map(|name| parse_id_tag(VAR_PREFIX, name).map(|id| (id, name)))
        {
            let kind = get_dvar_kind(var_name, integer, binary, real);
            let bound = get_dvar_bound(var_name, l, u);
            let id = VariableID::from(id_value);
            name_id_map.insert(var_name.clone(), id);
            let dvar = DecisionVariable::new(id, kind, bound, None, crate::ATol::default())
                .expect("Failed to create decision variable");
            dvars.insert(id, dvar);
        }
    }
    (dvars, name_id_map)
}

/// Strips the prefix of a variable/constraint name, and parses the following id number.
///
/// Returns none if prefix is not present, or if parsing as u64 fails.
fn parse_id_tag(prefix: &str, name: &str) -> Option<u64> {
    name.strip_prefix(prefix)?.parse().ok()
}

// name_id_map helps us convert from column name to id.
// See comment in `convert_dvars`
fn convert_objective(
    mps: &Mps,
    name_id_map: &HashMap<ColumnName, VariableID>,
) -> anyhow::Result<Function> {
    let Mps { b, c, .. } = mps;
    let constant = -b.get(&OBJ_NAME.into()).copied().unwrap_or_default();
    if c.is_empty() {
        Ok(Function::try_from(constant)?)
    } else {
        // Build linear function by adding terms
        let mut linear = crate::Linear::try_from(constant)?;
        for (name, &coefficient) in c {
            if let Some(&id) = name_id_map.get(name) {
                linear.add_term(
                    crate::LinearMonomial::Variable(id),
                    Coefficient::try_from(coefficient)?,
                );
            }
        }
        Ok(if linear.is_zero() {
            Function::Zero
        } else {
            Function::Linear(linear)
        })
    }
}

fn convert_constraints(
    mps: &Mps,
    name_id_map: &HashMap<ColumnName, VariableID>,
) -> anyhow::Result<BTreeMap<ConstraintID, Constraint>> {
    let Mps {
        a, b, eq, ge, le, ..
    } = mps;
    let mut constrs = BTreeMap::new();

    // as with decision variables, we're trying to recover IDs whenever all constraints match the naming scheme
    if a.keys().any(|name| !name.starts_with(CONSTR_PREFIX)) {
        // general case -- assign ids by order
        for (i, (row_name, row)) in a.iter().enumerate() {
            let b_value = b.get(row_name).copied().unwrap_or(0.0);
            let (function, equality) =
                convert_inequality(row, b_value, row_name, eq, ge, le, name_id_map)?;
            let id = ConstraintID::from(i as u64);
            let constraint = match equality {
                Equality::EqualToZero => Constraint::equal_to_zero(id, function),
                Equality::LessThanOrEqualToZero => {
                    Constraint::less_than_or_equal_to_zero(id, function)
                }
            };
            constrs.insert(id, constraint);
        }
    } else {
        // recover IDs case
        for (id_value, row_name, row) in a.iter().filter_map(|(row_name, row)| {
            parse_id_tag(CONSTR_PREFIX, row_name).map(|id| (id, row_name, row))
        }) {
            let b_value = b.get(row_name).copied().unwrap_or(0.0);
            let (function, equality) =
                convert_inequality(row, b_value, row_name, eq, ge, le, name_id_map)?;
            let id = ConstraintID::from(id_value);
            let constraint = match equality {
                Equality::EqualToZero => Constraint::equal_to_zero(id, function),
                Equality::LessThanOrEqualToZero => {
                    Constraint::less_than_or_equal_to_zero(id, function)
                }
            };
            constrs.insert(id, constraint);
        }
    }
    Ok(constrs)
}

/// Handles passing the `b` constant part to the left-hand side, as we only
/// accept the right-hand side being 0.0.
///
/// Returns the full function plus what the OMMX equality should be.
fn convert_inequality(
    row: &HashMap<ColumnName, f64>,
    mut b: f64,
    name: &RowName,
    eq: &HashSet<RowName>,
    ge: &HashSet<RowName>,
    le: &HashSet<RowName>,
    name_id_map: &HashMap<ColumnName, VariableID>,
) -> anyhow::Result<(Function, Equality)> {
    let mut negate = false;

    let equality = if eq.contains(name) {
        if b != 0. {
            b = -b;
        }
        Equality::EqualToZero
    } else if le.contains(name) {
        if b != 0. {
            b = -b;
        }
        Equality::LessThanOrEqualToZero
    } else if ge.contains(name) {
        // must multiply all terms by -1
        negate = true;
        Equality::LessThanOrEqualToZero
    } else {
        // unsure what to do -- just gonna assume equality
        if b != 0. {
            b = -b;
        }
        Equality::EqualToZero
    };

    let function = if row.is_empty() {
        Function::try_from(b)?
    } else {
        // Build linear function by adding terms
        let mut linear = crate::Linear::try_from(b)?;

        for (col_name, &coefficient) in row {
            if let Some(&id) = name_id_map.get(col_name) {
                let coeff = if negate { -coefficient } else { coefficient };
                linear.add_term(
                    crate::LinearMonomial::Variable(id),
                    Coefficient::try_from(coeff)?,
                );
            }
        }

        if linear.is_zero() {
            Function::Zero
        } else {
            Function::Linear(linear)
        }
    };

    Ok((function, equality))
}

fn convert_sense(sense: ObjSense) -> Sense {
    match sense {
        ObjSense::Min => Sense::Minimize,
        ObjSense::Max => Sense::Maximize,
    }
}

fn get_dvar_kind(
    name: &ColumnName,
    integer: &HashSet<ColumnName>,
    binary: &HashSet<ColumnName>,
    real: &HashSet<ColumnName>,
) -> DecisionVariableKind {
    if integer.contains(name) {
        DecisionVariableKind::Integer
    } else if binary.contains(name) {
        DecisionVariableKind::Binary
    } else if real.contains(name) {
        DecisionVariableKind::Continuous
    } else {
        DecisionVariableKind::Continuous
    }
}

fn get_dvar_bound(
    var_name: &ColumnName,
    l: &HashMap<ColumnName, f64>,
    u: &HashMap<ColumnName, f64>,
) -> Bound {
    let (lower, upper) = match (l.get(var_name), u.get(var_name)) {
        (Some(&lower), None) => (lower, f64::INFINITY),
        (None, Some(&upper)) => {
            if upper <= 0.0 {
                (f64::NEG_INFINITY, upper)
            } else {
                (0.0, upper)
            }
        }
        (Some(&lower), Some(&upper)) => (lower, upper),

        (None, None) => (0.0, f64::INFINITY),
    };
    Bound::new(lower, upper).expect("Invalid bound")
}
