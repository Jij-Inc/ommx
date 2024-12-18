use super::MpsWriteError;
use crate::{mps::ObjSense, v1};
use std::{collections::HashMap, io::Write};

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
/// Metadata like problem descriptions and variable/constraint names are not
/// preserved.
pub fn write_mps<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<(), MpsWriteError> {
    write_beginning(instance, out)?;
    write_rows(instance, out)?;
    write_columns(instance, out)?;
    write_rhs(instance, out)?;
    write_bounds(instance, out)?;
    writeln!(out, "ENDATA\n")?;
    Ok(())
}

fn write_beginning<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<(), MpsWriteError> {
    let name = instance
        .description
        .clone()
        .and_then(|descr| descr.name)
        .unwrap_or(String::from("Converted OMMX problem"));
    let obj_sense = match instance.sense {
        // v1::instance::Sense::Maximize
        // TODO more robust way to write this?
        2 => ObjSense::Max,
        _ => ObjSense::Min,
    };
    writeln!(out, "NAME {name}")?;
    writeln!(out, "OBJSENSE {obj_sense}")?;
    Ok(())
}

fn write_rows<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "ROWS")?;
    // each line must be ` Kind  constr_name`, and include objective
    writeln!(out, " N OBJ")?;
    // ommx instances are always <= 0 or = 0, so `Kind` will always be either N or L.
    for constr in instance.constraints.iter() {
        let kind = match constr.equality {
            // v1::Equality::LessThanEqualToZero
            2 => "L",
            // assuming EqualToZero when unspecified. Error instead?
            _ => "E",
        };
        let name = constr_name(constr);
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

fn write_columns<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "COLUMNS")?;
    let mut marker_tracker = IntorgTracker::default();
    for dvar in instance.decision_variables.iter() {
        let id = dvar.id;
        let var_name = dvar_name(dvar);
        match dvar.kind {
            // binary or integer var
            1 | 2 => marker_tracker.intorg(out)?,
            _ => marker_tracker.intend(out)?,
        }
        // write obj function entry
        write_col_entry(id, &var_name, OBJ_NAME, instance.objective().as_ref(), out)
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
        for constr in instance.constraints.iter() {
            let row_name = constr_name(constr);
            write_col_entry(id, &var_name, &row_name, constr.function().as_ref(), out)?;
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
    var_id: u64,
    var_name: &str,
    row_name: &str,
    func: &v1::Function,
    out: &mut W,
) -> Result<(), MpsWriteError> {
    if let Some(v1::Linear { terms, .. }) = func.clone().as_linear() {
        // search for current id in terms. If present and coefficient not 0, write entry
        for term in terms {
            if term.id == var_id && term.coefficient != 0.0 {
                let coeff = term.coefficient;
                writeln!(out, "    {var_name}  {row_name}  {coeff}")?;
            }
        }
    } else {
        return Err(MpsWriteError::InvalidConstraintType {
            name: row_name.to_string(),
            degree: func.degree(),
        });
    }
    Ok(())
}

fn write_rhs<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "RHS")?;
    // write out a RHS entry for the objective function if a non-zero constant is present
    if let Some(v1::Linear { constant, .. }) = instance.objective().into_owned().as_linear() {
        if constant != 0.0 {
            let rhs = -constant;
            writeln!(out, "  RHS1    {OBJ_NAME}   {rhs}")?;
        }
    }
    for constr in instance.constraints.iter() {
        let name = constr_name(constr);
        if let Some(v1::Linear { constant, .. }) = constr.function().into_owned().as_linear() {
            if constant != 0.0 {
                let rhs = -constant;
                writeln!(out, "  RHS1    {name}   {rhs}")?;
            }
        } else {
            return Err(MpsWriteError::InvalidConstraintType {
                name: name.to_string(),
                degree: constr.function().degree(),
            });
        }
    }
    Ok(())
}

fn write_bounds<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<(), MpsWriteError> {
    writeln!(out, "BOUNDS")?;
    // build an id -> dvar map as the vec is not guaranteed to be in order
    let var_by_id: HashMap<_, _> = instance
        .decision_variables
        .iter()
        .map(|var| (var.id, var))
        .collect();
    for dvar_id in instance.used_decision_variable_ids().into_iter() {
        let dvar = var_by_id
            .get(&dvar_id)
            .ok_or(MpsWriteError::InvalidVariableId(dvar_id))?;
        let name = dvar_name(dvar);
        if let Some(bound) = &dvar.bound {
            let (low_kind, up_kind) = match dvar.kind {
                // for now ignoring the BV specifier for binary variables
                // due to uncertainty in how widely supported it is.
                1 | 2 => ("LI", "UI"),
                _ => ("LO", "UP"),
            };
            writeln!(out, "  {up_kind} BND1    {name}  {}", bound.upper)?;
            writeln!(out, "  {low_kind} BND1    {name}  {}", bound.lower)?;
        };
    }
    Ok(())
}

/// Generates a name for the constraint based on its ID.
///
/// The constraint's name is ignored, if present.
fn constr_name(constr: &v1::Constraint) -> String {
    format!("{CONSTR_PREFIX}{}", constr.id)
}

/// Generates a name for the decision variable based on its ID.
///
/// The decision variable's name is ignored, if present.
fn dvar_name(dvar: &v1::DecisionVariable) -> String {
    format!("{VAR_PREFIX}{}", dvar.id)
}
