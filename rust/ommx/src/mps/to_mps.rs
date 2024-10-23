use crate::{
    mps::{Mps, ObjSense},
    v1,
};
use std::io::Write;

pub fn to_mps(instance: v1::Instance) -> Mps {
    let mut mps = Mps::default();
    mps.name = instance
        .description
        .map(|descr| descr.name.clone())
        .flatten()
        .unwrap_or(String::from("Converted OMMX problem"));
    // TODO fallible conversion?
    mps.obj_sense = match instance.sense {
        // v1::instance::Sense::Maximize
        // TODO more robust way to write this?
        2 => ObjSense::Max,
        _ => ObjSense::Min,
    };
    todo!()
}

fn write_mps<W: std::io::Write>(mps: &Mps, out: &mut W) -> anyhow::Result<()> {
    write_beginning(mps, out)?;
    write_rows(mps, out)?;
    write_columns(mps, out)?;
    write_rhs(mps, out)?;
    write_bounds(mps, out)?;
    write!(out, "ENDDATA\n")?;
    Ok(())
}

fn write_beginning<W: std::io::Write>(mps: &Mps, f: &mut W) -> anyhow::Result<()> {
    writeln!(f, "NAME {}", mps.name)?;
    write!(f, "OBJSENSE {}", mps.obj_sense)?;
    Ok(())
}

fn write_rows<W: Write>(mps: &Mps, out: &mut W) -> anyhow::Result<()> {
    todo!()
}
fn write_columns<W: Write>(mps: &Mps, out: &mut W) -> anyhow::Result<()> {
    todo!()
}
fn write_rhs<W: Write>(mps: &Mps, out: &mut W) -> anyhow::Result<()> {
    todo!()
}
fn write_bounds<W: Write>(mps: &Mps, out: &mut W) -> anyhow::Result<()> {
    todo!()
}
