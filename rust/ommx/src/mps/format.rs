use super::MpsWriteError;
use crate::decision_variable::Kind as DecisionVariableKind;
use crate::{mps::ObjSense, ConstraintID, Equality, Function, Instance, Sense, VariableID};
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
    let obj_sense = match *instance.sense() {
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
    for (var_id, dvar) in instance.decision_variables().iter() {
        let var_name = dvar_name(*var_id);
        match dvar.kind() {
            // binary or integer var
            DecisionVariableKind::Binary | DecisionVariableKind::Integer => {
                marker_tracker.intorg(out)?
            }
            _ => marker_tracker.intend(out)?,
        }
        // write obj function entry
        write_col_entry(*var_id, &var_name, OBJ_NAME, instance.objective(), out)
            // a bit of a workaround so that write_col_entry is easier to write.
            // It assumes we're dealing with constraints, but here we change the
            // error type so the message is clearer with the objective function
            .map_err(|err| {
                if let MpsWriteError::InvalidConstraintType { degree, .. } = err {
                    MpsWriteError::InvalidObjectiveType { degree }
                } else {
                    panic!() // we know this can't happen
                }
            })?;
        // write entries of this var's column for each constraint
        for (constr_id, constr) in instance.constraints().iter() {
            let row_name = constr_name(*constr_id);
            write_col_entry(*var_id, &var_name, &row_name, &constr.function, out)?;
        }
    }
    // print final INTEND
    marker_tracker.intend(out)?;
    Ok(())
}

/// Writes the entry in the COLUMNS sections of the given id and name, for the
/// corresponding row (constraint/obj function).
///
/// Only writes if var_id is part of the terms in the function, and only if
/// coefficient is not 0.
fn write_col_entry<W: Write>(
    var_id: VariableID,
    var_name: &str,
    row_name: &str,
    func: &Function,
    out: &mut W,
) -> Result<(), MpsWriteError> {
    if let Some(linear) = func.as_linear() {
        // get coefficient for the variable
        let linear_monomial = crate::LinearMonomial::Variable(var_id);
        if let Some(coeff) = linear.get(&linear_monomial) {
            let coeff_value: f64 = coeff.into();
            if coeff_value != 0.0 {
                writeln!(out, "    {var_name}  {row_name}  {coeff_value}")?;
            }
        }
    } else {
        return Err(MpsWriteError::InvalidConstraintType {
            name: row_name.to_string(),
            degree: (*func.degree()),
        });
    }
    Ok(())
}

fn write_rhs<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "RHS")?;
    // write out a RHS entry for the objective function if a non-zero constant is present
    if let Some(linear) = instance.objective().as_linear() {
        let constant: f64 = linear.constant_term();
        if constant != 0.0 {
            let rhs = -constant;
            writeln!(out, "  RHS1    {OBJ_NAME}   {rhs}")?;
        }
    }
    for (constr_id, constr) in instance.constraints().iter() {
        let name = constr_name(*constr_id);
        if let Some(linear) = constr.function.as_linear() {
            let constant: f64 = linear.constant_term();
            if constant != 0.0 {
                let rhs = -constant;
                writeln!(out, "  RHS1    {name}   {rhs}")?;
            }
        } else {
            return Err(MpsWriteError::InvalidConstraintType {
                name: name.to_string(),
                degree: (*constr.function.degree()),
            });
        }
    }
    Ok(())
}

fn write_bounds<W: Write>(instance: &Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "BOUNDS")?;

    for (var_id, dvar) in instance.decision_variables().iter() {
        let name = dvar_name(*var_id);
        let bound = dvar.bound();
        let (low_kind, up_kind) = match dvar.kind() {
            // for now ignoring the BV specifier for binary variables
            // due to uncertainty in how widely supported it is.
            DecisionVariableKind::Binary | DecisionVariableKind::Integer => ("LI", "UI"),
            _ => ("LO", "UP"),
        };
        writeln!(out, "  {up_kind} BND1    {name}  {}", bound.upper())?;
        writeln!(out, "  {low_kind} BND1    {name}  {}", bound.lower())?;
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
