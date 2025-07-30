use super::MpsWriteError;
use crate::decision_variable::Kind as DecisionVariableKind;
use crate::{
    mps::ObjSense, Coefficient, ConstraintID, Equality, Instance, Sense, VariableID, VariableIDSet,
};
use std::collections::BTreeMap;
use std::io::Write;

pub(crate) const OBJ_NAME: &str = "OBJ";
pub(crate) const CONSTR_PREFIX: &str = "OMMX_CONSTR_";
pub(crate) const VAR_PREFIX: &str = "OMMX_VAR_";

/// Writes out the instance in MPS format to the specified `Write`r.
///
/// This function does not automatically Gzip the output -- that is the
/// responsibility of the Write implementation.
///
/// Only linear problems are supported.
///
/// ## Information Loss and Filtering
///
/// Metadata like problem descriptions and variable/constraint names are not
/// preserved.
///
/// **Removed Constraints**: All `removed_constraints` are completely ignored
/// and not written to the MPS file. The MPS format cannot represent the
/// concept of removed constraints, so this information is lost during export.
///
/// **Variable Filtering**: Only decision variables that are actually used in
/// the objective function, active constraints, or removed constraints are
/// written to the MPS file. Variables defined in `decision_variables` but
/// not referenced anywhere are omitted from the output. This is determined
/// by the `required_ids()` method which includes:
/// - Variables used in the objective function
/// - Variables used in active constraints
/// - Variables used in removed constraints (even though the constraints
///   themselves are not exported)
///
/// This ensures that variables from removed constraints are preserved in
/// the MPS output even though the constraint information is lost.
pub fn format<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    write_beginning(instance, out)?;
    write_rows(instance, out)?;
    write_columns(instance, out)?;
    write_rhs(instance, out)?;
    write_bounds(instance, out)?;
    write_quadobj(instance, out)?;
    write_qcmatrix(instance, out)?;
    writeln!(out, "ENDATA\n")?;
    Ok(())
}

/// Converts the instance to a string in MPS format via [`format()`].
pub fn to_string(instance: &Instance) -> Result<String, MpsWriteError> {
    let mut buffer = Vec::new();
    format(instance, &mut buffer)?;
    Ok(String::from_utf8(buffer).unwrap())
}

fn write_beginning<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    let name = instance
        .description
        .clone()
        .and_then(|descr| descr.name)
        .unwrap_or(String::from("Converted OMMX problem"));
    let obj_sense = match instance.sense() {
        Sense::Maximize => ObjSense::Max,
        Sense::Minimize => ObjSense::Min,
    };
    writeln!(out, "NAME {name}")?;
    writeln!(out, "OBJSENSE {obj_sense}")?;
    Ok(())
}

fn write_rows<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "ROWS")?;
    // each line must be ` Kind  constr_name`, and include objective
    writeln!(out, " N OBJ")?;
    // ommx instances are always <= 0 or = 0, so `Kind` will always be either N or L.
    for (id, constr) in instance.constraints().iter() {
        let kind = match constr.equality {
            Equality::LessThanOrEqualToZero => "L",
            // assuming EqualToZero when unspecified. Error instead?
            _ => "E",
        };
        let name = constr_name(*id);
        writeln!(out, " {kind} {name}")?;
    }
    Ok(())
}

#[derive(Default)]
struct IntorgTracker {
    intorg_block: bool,
    counter: u64,
}

impl IntorgTracker {
    fn intorg<W: Write>(&mut self, out: &mut W) -> Result<(), MpsWriteError> {
        // only print marker if not already in INTORG block
        if !self.intorg_block {
            self.intorg_block = true;
            writeln!(out, "    MARK{}   'MARKER'      'INTORG'", self.counter)?;
            self.counter += 1;
        }
        Ok(())
    }
    fn intend<W: Write>(&mut self, out: &mut W) -> Result<(), MpsWriteError> {
        // only print marker if in INTORG block
        if self.intorg_block {
            self.intorg_block = false;
            writeln!(out, "    MARK{}   'MARKER'      'INTEND'", self.counter)?;
            self.counter += 1;
        }
        Ok(())
    }
}

fn write_columns<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "COLUMNS")?;
    let mut marker_tracker = IntorgTracker::default();

    // Collect all linear terms from objective and constraints
    // Structure: VariableID -> Vec<(row_name, coefficient)>
    let mut variable_entries: BTreeMap<VariableID, Vec<(String, Coefficient)>> = BTreeMap::new();

    // Collect linear terms from objective function
    for (var_id, coeff) in instance.objective().linear_terms() {
        variable_entries
            .entry(var_id)
            .or_default()
            .push((OBJ_NAME.to_string(), coeff));
    }

    // Collect linear terms from constraints
    for (constr_id, constr) in instance.constraints().iter() {
        let row_name = constr_name(*constr_id);
        for (var_id, coeff) in constr.function.linear_terms() {
            variable_entries
                .entry(var_id)
                .or_default()
                .push((row_name.clone(), coeff));
        }
    }

    // Write columns for variables with linear terms
    let mut written_variables = VariableIDSet::new();
    for (var_id, entries) in variable_entries {
        let dvar = instance
            .decision_variables()
            .get(&var_id)
            .expect("Variable ID from linear_terms() must exist in decision_variables");
        written_variables.insert(var_id);
        let var_name = dvar_name(var_id);

        match dvar.kind() {
            // binary or integer var
            DecisionVariableKind::Binary | DecisionVariableKind::Integer => {
                marker_tracker.intorg(out)?
            }
            _ => marker_tracker.intend(out)?,
        }

        // Write all entries for this variable
        for (row_name, coeff) in entries {
            let coeff_value: f64 = coeff.into();
            if coeff_value != 0.0 {
                writeln!(out, "    {var_name}  {row_name}  {coeff_value}")?;
            }
        }
    }

    // Second pass: write variables that only appear in quadratic terms (with zero coefficient)
    let used_ids = instance.used_decision_variable_ids();
    for var_id in used_ids.difference(&written_variables) {
        let dvar = instance.decision_variables().get(var_id).expect(
            "Variable ID from used_decision_variable_ids() must exist in decision_variables",
        );
        let var_name = dvar_name(*var_id);
        match dvar.kind() {
            // binary or integer var
            DecisionVariableKind::Binary | DecisionVariableKind::Integer => {
                marker_tracker.intorg(out)?
            }
            _ => marker_tracker.intend(out)?,
        }
        // Write dummy entry with coefficient 0 for OBJ
        writeln!(out, "    {var_name}  {OBJ_NAME}  0")?;
    }

    // print final INTEND
    marker_tracker.intend(out)?;
    Ok(())
}

fn write_rhs<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "RHS")?;
    // write out a RHS entry for the objective function if a non-zero constant is present
    let constant = if let Some(linear) = instance.objective().as_linear() {
        linear.constant_term()
    } else if let Some(quadratic) = instance.objective().as_quadratic() {
        quadratic.constant_term()
    } else {
        // Higher degree functions not supported
        return Err(MpsWriteError::InvalidObjectiveType {
            degree: (*instance.objective().degree()),
        });
    };

    if constant != 0.0 {
        let rhs = -constant;
        writeln!(out, "  RHS1    {OBJ_NAME}   {rhs}")?;
    }

    for (constr_id, constr) in instance.constraints().iter() {
        let name = constr_name(*constr_id);
        let constant = if let Some(linear) = constr.function.as_linear() {
            linear.constant_term()
        } else if let Some(quadratic) = constr.function.as_quadratic() {
            quadratic.constant_term()
        } else {
            // Higher degree functions not supported
            return Err(MpsWriteError::InvalidConstraintType {
                name: name.to_string(),
                degree: (*constr.function.degree()),
            });
        };

        if constant != 0.0 {
            let rhs = -constant;
            writeln!(out, "  RHS1    {name}   {rhs}")?;
        }
    }
    Ok(())
}

fn write_bounds<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "BOUNDS")?;

    for (var_id, dvar) in instance.used_decision_variables() {
        let name = dvar_name(var_id);
        let bound = dvar.bound();

        // Check special cases for infinity bounds
        if bound.lower() == f64::NEG_INFINITY && bound.upper() == f64::INFINITY {
            // Unbounded variable (-inf, inf)
            writeln!(out, "  FR BND1    {name}")?;
        } else if bound.lower() == f64::NEG_INFINITY {
            // Lower bound is -inf, upper bound is finite
            writeln!(out, "  MI BND1    {name}")?;
            let up_kind = match dvar.kind() {
                DecisionVariableKind::Binary | DecisionVariableKind::Integer => "UI",
                _ => "UP",
            };
            writeln!(out, "  {up_kind} BND1    {name}  {}", bound.upper())?;
        } else if bound.upper() == f64::INFINITY {
            // Upper bound is +inf, lower bound is finite
            writeln!(out, "  PL BND1    {name}")?;
            let low_kind = match dvar.kind() {
                DecisionVariableKind::Binary | DecisionVariableKind::Integer => "LI",
                _ => "LO",
            };
            writeln!(out, "  {low_kind} BND1    {name}  {}", bound.lower())?;
        } else {
            // Both bounds are finite
            let (low_kind, up_kind) = match dvar.kind() {
                // for now ignoring the BV specifier for binary variables
                // due to uncertainty in how widely supported it is.
                DecisionVariableKind::Binary | DecisionVariableKind::Integer => ("LI", "UI"),
                _ => ("LO", "UP"),
            };
            writeln!(out, "  {up_kind} BND1    {name}  {}", bound.upper())?;
            writeln!(out, "  {low_kind} BND1    {name}  {}", bound.lower())?;
        }
    }
    Ok(())
}

/// Generates a name for the constraint based on its ID.
///
/// The constraint's name is ignored, if present.
fn constr_name(constr_id: ConstraintID) -> String {
    format!("{CONSTR_PREFIX}{}", constr_id.into_inner())
}

/// Generates a name for the decision variable based on its ID.
///
/// The decision variable's name is ignored, if present.
fn dvar_name(var_id: VariableID) -> String {
    format!("{VAR_PREFIX}{}", var_id.into_inner())
}

fn write_quadobj<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    // Only write QUADOBJ section if the objective has quadratic terms
    if let Some(quadratic) = instance.objective().as_quadratic() {
        let has_quadratic_terms = quadratic
            .iter()
            .any(|(monomial, _)| matches!(monomial, crate::QuadraticMonomial::Pair(_)));

        if has_quadratic_terms {
            writeln!(out, "QUADOBJ")?;

            // Write quadratic terms in sorted order for deterministic output
            let mut quadratic_terms: Vec<_> = quadratic
                .iter()
                .filter_map(|(monomial, coeff)| {
                    if let crate::QuadraticMonomial::Pair(pair) = monomial {
                        Some((pair, coeff))
                    } else {
                        None
                    }
                })
                .collect();

            quadratic_terms.sort_by_key(|(pair, _)| (pair.lower(), pair.upper()));

            for (pair, coeff) in quadratic_terms {
                let var1_name = dvar_name(pair.lower());
                let var2_name = dvar_name(pair.upper());
                let coeff_value: f64 = (*coeff).into();
                if coeff_value != 0.0 {
                    writeln!(out, "    {var1_name}  {var2_name}  {coeff_value}")?;
                }
            }
        }
    }
    Ok(())
}

fn write_qcmatrix<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    // Write QCMATRIX sections for each constraint that has quadratic terms
    for (constr_id, constr) in instance.constraints().iter() {
        if let Some(quadratic) = constr.function.as_quadratic() {
            let has_quadratic_terms = quadratic
                .iter()
                .any(|(monomial, _)| matches!(monomial, crate::QuadraticMonomial::Pair(_)));

            if has_quadratic_terms {
                let constraint_name = constr_name(*constr_id);
                writeln!(out, "QCMATRIX {constraint_name}")?;

                // Write quadratic terms in sorted order for deterministic output
                let mut quadratic_terms: Vec<_> = quadratic
                    .iter()
                    .filter_map(|(monomial, coeff)| {
                        if let crate::QuadraticMonomial::Pair(pair) = monomial {
                            Some((pair, coeff))
                        } else {
                            None
                        }
                    })
                    .collect();

                quadratic_terms.sort_by_key(|(pair, _)| (pair.lower(), pair.upper()));

                for (pair, coeff) in quadratic_terms {
                    let var1_name = dvar_name(pair.lower());
                    let var2_name = dvar_name(pair.upper());
                    let coeff_value: f64 = (*coeff).into();
                    if coeff_value != 0.0 {
                        writeln!(out, "    {var1_name}  {var2_name}  {coeff_value}")?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decision_variable::Kind, Bound, DecisionVariable, Function};
    use maplit::btreemap;

    #[test]
    fn test_write_bounds_unbounded() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                VariableID::from(0),
                Kind::Continuous,
                Bound::unbounded(),
                None,
                crate::ATol::default()
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          FR BND1    OMMX_VAR_0
        "###);
    }

    #[test]
    fn test_write_bounds_positive() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                VariableID::from(0),
                Kind::Continuous,
                Bound::positive(),
                None,
                crate::ATol::default()
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          PL BND1    OMMX_VAR_0
          LO BND1    OMMX_VAR_0  0
        "###);
    }

    #[test]
    fn test_write_bounds_negative() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                VariableID::from(0),
                Kind::Continuous,
                Bound::negative(),
                None,
                crate::ATol::default()
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          MI BND1    OMMX_VAR_0
          UP BND1    OMMX_VAR_0  0
        "###);
    }

    #[test]
    fn test_write_bounds_integer_types() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                VariableID::from(0),
                Kind::Binary,
                Bound::of_binary(),
                None,
                crate::ATol::default()
            ).unwrap(),
            VariableID::from(1) => DecisionVariable::new(
                VariableID::from(1),
                Kind::Integer,
                Bound::new(-10.0, 20.0).unwrap(),
                None,
                crate::ATol::default()
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0) + crate::linear!(1)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          UI BND1    OMMX_VAR_0  1
          LI BND1    OMMX_VAR_0  0
          UI BND1    OMMX_VAR_1  20
          LI BND1    OMMX_VAR_1  -10
        "###);
    }

    #[test]
    fn test_write_bounds_mixed_types() {
        let decision_variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                VariableID::from(0),
                Kind::Continuous,
                Bound::unbounded(),
                None,
                crate::ATol::default()
            ).unwrap(),
            VariableID::from(1) => DecisionVariable::new(
                VariableID::from(1),
                Kind::Continuous,
                Bound::positive(),
                None,
                crate::ATol::default()
            ).unwrap(),
            VariableID::from(2) => DecisionVariable::new(
                VariableID::from(2),
                Kind::Integer,
                Bound::negative(),
                None,
                crate::ATol::default()
            ).unwrap(),
        };

        let instance = Instance::new(
            Sense::Minimize,
            Function::from(crate::linear!(0) + crate::linear!(1) + crate::linear!(2)),
            decision_variables,
            btreemap! {},
        )
        .unwrap();

        let mut buffer = Vec::new();
        write_bounds(&instance, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        insta::assert_snapshot!(output, @r###"
        BOUNDS
          FR BND1    OMMX_VAR_0
          PL BND1    OMMX_VAR_1
          LO BND1    OMMX_VAR_1  0
          MI BND1    OMMX_VAR_2
          UI BND1    OMMX_VAR_2  0
        "###);
    }
}
