use crate::{
    mps::{Mps, ObjSense},
    v1,
};
use std::io::Write;

fn write_mps<W: std::io::Write>(mps: &v1::Instance, out: &mut W) -> anyhow::Result<()> {
    write_beginning(mps, out)?;
    write_rows(mps, out)?;
    write_columns(mps, out)?;
    write_rhs(mps, out)?;
    write_bounds(mps, out)?;
    writeln!(out, "ENDDATA\n")?;
    Ok(())
}

fn write_beginning<W: std::io::Write>(instance: &v1::Instance, out: &mut W) -> anyhow::Result<()> {
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

fn write_rows<W: Write>(instance: &v1::Instance, out: &mut W) -> anyhow::Result<()> {
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
        let name = constr
            .name
            .clone()
            .unwrap_or_else(|| format!("constr_id{}", constr.id));
        writeln!(out, " {kind} {name}")?;
    }
    Ok(())
}
fn write_columns<W: Write>(instance: &v1::Instance, out: &mut W) -> anyhow::Result<()> {
    todo!()
}
fn write_rhs<W: Write>(instance: &v1::Instance, out: &mut W) -> anyhow::Result<()> {
    todo!()
}
fn write_bounds<W: Write>(instance: &v1::Instance, out: &mut W) -> anyhow::Result<()> {
    todo!()
}
