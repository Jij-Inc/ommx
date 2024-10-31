use crate::{
    mps::{Mps, ObjSense},
    v1,
};
use anyhow::Result;
use std::{borrow::Cow, io::Write};

fn write_mps<W: std::io::Write>(mps: &v1::Instance, out: &mut W) -> Result<()> {
    write_beginning(mps, out)?;
    write_rows(mps, out)?;
    write_columns(mps, out)?;
    write_rhs(mps, out)?;
    write_bounds(mps, out)?;
    writeln!(out, "ENDATA\n")?;
    Ok(())
}

fn write_beginning<W: std::io::Write>(instance: &v1::Instance, out: &mut W) -> Result<()> {
    let name = instance
        .description
        .clone()
        .map(|descr| descr.name)
        .flatten()
        .unwrap_or(String::from("Converted OMMX problem"));
    // TODO fallible conversion?
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

fn write_rows<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<()> {
    writeln!(out, "ROWS")?;
    // each line must be ` Kind  constr_name`, and include objective
    writeln!(out, " N OBJ")?;
    // ommx instances are always <= 0 or = 0, so `Kind` will always be either N or L.
    for constr in instance.constraints.iter() {
        let kind = match constr.equality {
            // v1::Equality::LessThanEqualToZero
            2 => "L",
            // assuming EqualToZero when unspecified. Error instead?
            _ => "N",
        };
        let name = constr_name(constr);
        writeln!(out, "  {kind} {name}")?;
    }
    Ok(())
}

fn write_columns<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<()> {
    writeln!(out, "RHS")?;
    let obj_name = Cow::Borrowed("OBJ");
    for dvar in instance.decision_variables.iter() {
        let id = dvar.id;
        let var_name = dvar_name(dvar);
        write_col_entry(id, &var_name, &obj_name, instance.objective.as_ref(), out)?;
        for constr in instance.constraints.iter() {
            let row_name = constr_name(constr);
            write_col_entry(id, &var_name, &row_name, constr.function.as_ref(), out)?;
        }
    }
    Ok(())
}

/// Writes the entry in the COLUMNS sections of the given id and name, for the
/// corresponding row (constraint/obj function).
///
/// Only writes if var_id is part of the terms in the function, and only if
/// coefficient is not 0.
fn write_col_entry<W: Write>(
    var_id: u64,
    var_name: &Cow<str>,
    row_name: &Cow<str>,
    func: Option<&v1::Function>,
    out: &mut W,
) -> Result<()> {
    let Some(v1::Function {
        function: Some(func),
    }) = &func
    else {
        return Ok(());
    };
    match func {
        v1::function::Function::Constant(_) => (),

        v1::function::Function::Quadratic(_) => todo!(), // error out
        v1::function::Function::Polynomial(_) => todo!(), // error out
        v1::function::Function::Linear(v1::Linear { terms, .. }) => {
            // search for current id in terms. If present and coefficient not 0, write entry
            for term in terms {
                if term.id == var_id && term.coefficient != 0.0 {
                    let coeff = term.coefficient;
                    writeln!(out, "  {var_name}  {row_name}  {coeff}")?;
                }
            }
        }
    }
    Ok(())
}

fn write_rhs<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<()> {
    writeln!(out, "RHS")?;
    for constr in instance.constraints.iter() {
        let name = constr_name(constr);
        // let name = constr
        //     .name
        //     .clone()
        //     .unwrap_or_else(|| format!("constr_id{}", constr.id));
        // all are 0. We could potentially omit this section entirely?
        writeln!(out, "    RHS1    {name} 0")?;
    }
    Ok(())
}

fn write_bounds<W: Write>(instance: &v1::Instance, out: &mut W) -> Result<()> {
    writeln!(out, "BOUNDS")?;
    for dvar in instance.decision_variables.iter() {
        let name = dvar_name(dvar);
        if let Some(bound) = &dvar.bound {
            let (low_kind, up_kind) = match dvar.kind {
                // for now ignoring the BV specifier for binary variables
                // due to uncertainty in how widely supported it is.
                1 | 2 => ("LI", "UI"),
                _ => ("LO", "UP"),
            };
            writeln!(out, " {up_kind} BND1    {name}  {}", bound.upper)?;
            writeln!(out, " {low_kind} BND1    {name}  {}", bound.lower)?;
        };
    }
    Ok(())
}

/// Either returns a borrowed name of the constraint if present or
/// generates a name based on the id.
fn constr_name(constr: &v1::Constraint) -> Cow<str> {
    match &constr.name {
        Some(name) => Cow::Borrowed(&name),
        None => Cow::Owned(format!("constr_id{}", constr.id)),
    }
}

/// Either returns a borrowed name of the decision variable if present or
/// generates a name based on the id.
fn dvar_name(dvar: &v1::DecisionVariable) -> Cow<str> {
    match &dvar.name {
        Some(name) => Cow::Borrowed(&name),
        None => Cow::Owned(format!("dvar_id{}", dvar.id)),
    }
}
