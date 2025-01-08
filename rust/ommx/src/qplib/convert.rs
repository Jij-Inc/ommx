use itertools::izip;
use num::Zero;

use super::{
    parser::{ObjSense, QplibFile, VarType},
    QplibParseError,
};
use crate::v1;
use std::collections::HashMap;

pub fn convert(mut qplib: QplibFile) -> Result<v1::Instance, QplibParseError> {
    qplib.apply_infinity_threshold();
    let description = convert_description(&qplib);
    let decision_variables = convert_dvars(&qplib);
    let objective = convert_objective(&qplib);
    let constraints = convert_constraints(&qplib);
    Ok(v1::Instance {
        description: Some(description),
        decision_variables,
        objective: Some(objective),
        constraints,
        sense: convert_sense(qplib.sense),
        ..Default::default()
    })
}

fn convert_description(qplib: &QplibFile) -> v1::instance::Description {
    // currently only gets the name
    let name = if qplib.name.is_empty() {
        None
    } else {
        Some(qplib.name.clone())
    };
    let description = qplib.problem_type.to_string();
    v1::instance::Description {
        name,
        description: Some(description),
        ..Default::default()
    }
}

fn convert_dvars(qplib: &QplibFile) -> Vec<v1::DecisionVariable> {
    let QplibFile {
        var_types,
        lower_bounds,
        upper_bounds,
        var_names,
        ..
    } = qplib;
    let mut dvars = Vec::with_capacity(var_types.len());
    for (i, (t, &lower, &upper)) in izip!(var_types, lower_bounds, upper_bounds).enumerate() {
        let id = i as u64;
        let name = var_names.get(&i).cloned();
        let kind = match t {
            VarType::Continuous => v1::decision_variable::Kind::Continuous as i32,
            VarType::Integer => v1::decision_variable::Kind::Integer as i32,
            VarType::Binary => v1::decision_variable::Kind::Binary as i32,
        };
        let bound = v1::Bound { lower, upper };
        dvars.push(v1::DecisionVariable {
            id,
            kind,
            bound: Some(bound),
            name,
            ..Default::default()
        })
    }
    dvars
}

fn convert_objective(qplib: &QplibFile) -> v1::Function {
    let quadratic = to_quadratic(&qplib.q0_non_zeroes);
    let linear = if qplib.default_b0 == 0.0 {
        to_linear(&qplib.b0_non_defaults)
    } else {
        // non-zero default value: transform into dense vec using the default value and update
        // with non-defaults
        let mut terms: Vec<_> = (0..qplib.num_vars as u64)
            .map(|i| v1::linear::Term {
                id: i,
                coefficient: qplib.default_b0,
            })
            .collect();
        for (&i, &coeff) in qplib.b0_non_defaults.iter() {
            terms[i].coefficient = coeff;
        }
        // remove any terms which may have been explicitly set to 0.0
        terms.retain(|t| t.coefficient != 0.0);
        v1::Linear {
            terms,
            constant: 0.0, // constant will be added later
        }
    };
    // simplify function to linear/constant if appropriate terms not
    // present
    wrap_function(quadratic, linear, qplib.obj_constant)
}

fn convert_constraints(qplib: &QplibFile) -> Vec<v1::Constraint> {
    let QplibFile {
        num_constraints,
        qs_non_zeroes,
        bs_non_zeroes,
        constr_lower_cs,
        constr_upper_cs,
        constr_names,
        ..
    } = qplib;
    // a dummy default coefficient map for when constraints are only linear
    let default_q = HashMap::default();
    // our output Vec.
    // technically num_constraints is only a lower bound on the capacity
    // required as one Qplib constraint might equal 2 ommx constraints.
    let mut constraints = Vec::with_capacity(*num_constraints);

    for (i, (bs, &lower_c, &upper_c)) in
        izip!(bs_non_zeroes, constr_lower_cs, constr_upper_cs).enumerate()
    {
        // if the problem has only linear constraints, qs_non_zeroes will be empty
        let qs = qs_non_zeroes.get(i).unwrap_or(&default_q);
        let mut quadratic = to_quadratic(qs);
        let mut linear = to_linear(bs);
        let name = constr_names
            .get(&i)
            .cloned()
            .unwrap_or_else(|| format!("Qplib_constr_{i}"));
        // QPLIB constraints are two-sided, as in, `c_l <= expr <= c_u`.
        // To represent a one-sided constraint, c_l or c_u are set to the infinity
        // threshold. This means a single constraint translates to potentially 2
        // OMMX constraints.
        //
        // Currently we don't perform any checks for equality constraints
        // (represented in QPLIB by setting `c_l` and `c_u` to the same value)
        //
        // We don't create a `v1::Constraint` for sides where `c` is infinity.
        //
        // ID translation scheme (subject to potential change):
        //
        // - The `<= c_u` side is given the same ID as the constraint in the
        // QPLIB file.
        //
        // - The `>= c_l` side is given a new ID, which is `num_constraints +
        // id`. Hence in a QPLIB file with 10 constraints, the `c_l` side of
        // constraint 0 is 10; for constraint 1 it's 11, and so on.
        //
        // This scheme means the resulting OMMX instance will have
        // non-contiguous constraint IDs if some constraints have no valid `<=
        // c_u` side.
        if upper_c != f64::INFINITY {
            // move upper_c to the LHS multiplied by -1.
            let func = wrap_function(quadratic.clone(), linear.clone(), -upper_c);
            constraints.push(v1::Constraint {
                id: i as u64,
                equality: v1::Equality::LessThanOrEqualToZero as i32,
                function: Some(func),
                name: Some(format!("{name} [c_u]")),
                ..Default::default()
            });
        }

        if lower_c != f64::NEG_INFINITY {
            // multiply ALL coefficients by -1, move constant to LHS
            quadratic.values.iter_mut().for_each(|v| *v *= -1.);
            linear.terms.iter_mut().for_each(|t| t.coefficient *= -1.);
            let func = wrap_function(quadratic, linear, lower_c);
            constraints.push(v1::Constraint {
                id: (num_constraints + i) as u64,
                equality: v1::Equality::LessThanOrEqualToZero as i32,
                function: Some(func),
                name: Some(format!("{name} [c_l]")),
                ..Default::default()
            });
        }
    }
    constraints
}

fn to_quadratic(coeff_map: &HashMap<(usize, usize), f64>) -> v1::Quadratic {
    let mut rows = Vec::with_capacity(coeff_map.len());
    let mut columns = Vec::with_capacity(coeff_map.len());
    let mut values = Vec::with_capacity(coeff_map.len());
    for ((row, col), val) in coeff_map.iter() {
        rows.push(*row as u64);
        columns.push(*col as u64);
        values.push(*val);
    }
    v1::Quadratic {
        rows,
        columns,
        values,
        linear: None,
    }
}

fn to_linear(coeffs: &HashMap<usize, f64>) -> v1::Linear {
    let terms: Vec<_> = coeffs
        .iter()
        .map(|(id, coeff)| v1::linear::Term {
            id: *id as u64,
            coefficient: *coeff,
        })
        .collect();
    v1::Linear {
        terms,
        constant: 0.0,
    }
}

fn wrap_function(mut quad: v1::Quadratic, mut linear: v1::Linear, constant: f64) -> v1::Function {
    let func = if quad.is_zero() {
        if linear.terms.is_empty() {
            v1::function::Function::Constant(constant)
        } else {
            linear.constant = constant;
            v1::function::Function::Linear(linear)
        }
    } else {
        linear.constant = constant;
        quad.linear = Some(linear);
        v1::function::Function::Quadratic(quad)
    };
    v1::Function {
        function: Some(func),
    }
}

fn convert_sense(sense: ObjSense) -> i32 {
    match sense {
        ObjSense::Minimize => v1::instance::Sense::Minimize as i32,
        ObjSense::Maximize => v1::instance::Sense::Maximize as i32,
    }
}
