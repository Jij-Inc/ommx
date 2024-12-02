use std::collections::{HashMap, HashSet};

use super::{
    parser::{ColumnName, ObjSense, RowName},
    Mps, MpsParseError,
};
use crate::v1;

pub fn convert(mps: Mps) -> Result<v1::Instance, MpsParseError> {
    let (decision_variables, name_id_map) = convert_dvars(&mps);
    let objective = convert_objective(&mps, &name_id_map);
    let constraints = convert_constraints(&mps, &name_id_map);
    Ok(v1::Instance {
        description: None, // TODO add description?
        decision_variables,
        objective: Some(objective),
        constraints,
        sense: convert_sense(mps.obj_sense),
        parameters: None,
    })
}

fn convert_dvars(mps: &Mps) -> (Vec<v1::DecisionVariable>, HashMap<ColumnName, u64>) {
    let Mps {
        vars,
        u,
        l,
        integer,
        binary,
        real,
        ..
    } = mps;
    let mut dvars = Vec::with_capacity(u.len());
    // Will be used to keep track of dvar ids throughout the conversion. might
    // not be strictly necessary if we make it so parser.rs and this file
    // guarantee a strict and consistent ordering, but it's less error-prone
    // this way
    let mut name_id_map = HashMap::with_capacity(u.len());

    for (i, var_name) in vars.iter().enumerate() {
        let kind = get_dvar_kind(var_name, integer, binary, real);
        let bound = get_dvar_bound(var_name, l, u);
        // our ID ends up being dependent on the order of vars hashset. This is
        // unstable across executions -- we might want to consider an indexset
        // in the future
        let id = i as u64;
        name_id_map.insert(var_name.clone(), id);
        dvars.push(v1::DecisionVariable {
            id: i as u64,
            kind,
            bound: Some(bound),
            name: Some(var_name.0.clone()),
            subscripts: Vec::new(),
            parameters: HashMap::new(),
            ..Default::default()
        })
    }
    (dvars, name_id_map)
}

// name_id_map helps us convert from column name to id.
// See comment in `convert_dvars`
fn convert_objective(mps: &Mps, name_id_map: &HashMap<ColumnName, u64>) -> v1::Function {
    let Mps { c, .. } = mps;
    let terms = convert_terms(c, name_id_map);
    let function = if terms.is_empty() {
        v1::function::Function::Constant(0.0)
    } else {
        v1::function::Function::Linear(v1::Linear {
            terms,
            constant: 0.0,
        })
    };
    v1::Function {
        function: Some(function),
    }
}

fn convert_terms(
    columns: &HashMap<ColumnName, f64>,
    name_id_map: &HashMap<ColumnName, u64>,
) -> Vec<v1::linear::Term> {
    columns
        .iter()
        .map(|(name, &coefficient)| {
            let id = name_id_map[name];
            v1::linear::Term { id, coefficient }
        })
        .collect()
}

fn convert_constraints(mps: &Mps, name_id_map: &HashMap<ColumnName, u64>) -> Vec<v1::Constraint> {
    let Mps {
        a, b, eq, ge, le, ..
    } = mps;
    let mut constrs = Vec::with_capacity(a.len());
    for (i, (row_name, row)) in a.iter().enumerate() {
        let b = b.get(row_name).copied().unwrap_or(0.0);
        let terms = convert_terms(row, name_id_map);
        let (function, equality) = convert_inequality(terms, b, row_name, eq, ge, le);
        constrs.push(v1::Constraint {
            id: i as u64,
            equality,
            function: Some(function),
            subscripts: Vec::new(),
            parameters: HashMap::new(),
            name: Some(row_name.0.clone()),
            description: None,
        })
    }
    constrs
}

/// Handles passing the `b` constant part to the left-hand side, as we only
/// accept the right-hand side being 0.0.
///
/// Returns the full function plus what the OMMX equality should be.
fn convert_inequality(
    mut terms: Vec<v1::linear::Term>,
    mut b: f64,
    name: &RowName,
    eq: &HashSet<RowName>,
    ge: &HashSet<RowName>,
    le: &HashSet<RowName>,
) -> (v1::Function, i32) {
    let equality = if eq.contains(name) {
        b = -b;
        v1::Equality::EqualToZero as i32
    } else if le.contains(name) {
        b = -b;
        v1::Equality::LessThanOrEqualToZero as i32
    } else if ge.contains(name) {
        // must multiply all terms by -1
        terms.iter_mut().for_each(|t| t.coefficient *= -1.);
        v1::Equality::LessThanOrEqualToZero as i32
    } else {
        // unsure what to do -- just gonna assume equality
        b = -b;
        v1::Equality::Unspecified as i32
    };

    let function = if terms.is_empty() {
        v1::function::Function::Constant(b)
    } else {
        v1::function::Function::Linear(v1::Linear { terms, constant: b })
    };
    (
        v1::Function {
            function: Some(function),
        },
        equality,
    )
}

fn convert_sense(sense: ObjSense) -> i32 {
    match sense {
        ObjSense::Min => v1::instance::Sense::Minimize as i32,
        ObjSense::Max => v1::instance::Sense::Maximize as i32,
    }
}

fn get_dvar_kind(
    name: &ColumnName,
    integer: &HashSet<ColumnName>,
    binary: &HashSet<ColumnName>,
    real: &HashSet<ColumnName>,
) -> i32 {
    if integer.contains(name) {
        v1::decision_variable::Kind::Integer as i32
    } else if binary.contains(name) {
        v1::decision_variable::Kind::Binary as i32
    } else if real.contains(name) {
        v1::decision_variable::Kind::Continuous as i32
    } else {
        v1::decision_variable::Kind::Unspecified as i32
    }
}

fn get_dvar_bound(
    var_name: &ColumnName,
    l: &HashMap<ColumnName, f64>,
    u: &HashMap<ColumnName, f64>,
) -> v1::Bound {
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
    v1::Bound { lower, upper }
}
