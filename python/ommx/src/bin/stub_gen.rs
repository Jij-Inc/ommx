use pyo3_stub_gen::Result;
use std::{path::Path, process::Command};

fn main() -> Result<()> {
    let stub = _ommx_rust::stub_info()?;
    stub.generate()?;

    let root: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    Command::new("ruff")
        .arg("format")
        .arg(root.join("ommx/_ommx_rust/__init__.pyi"))
        .arg(root.join("ommx/v1/__init__.py"))
        .status()?;
    Ok(())
}
