use anyhow::Result;
use colored::Colorize;
use prost_build::Config;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn main() -> Result<()> {
    let manifest_root: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let repo_root = manifest_root.join("../..").canonicalize()?;
    let proto_root = repo_root.join("protobuf");

    let protos = fs::read_dir(&proto_root)?
        .map(|entry| -> Result<PathBuf> { Ok(entry?.path()) })
        .collect::<Result<Vec<_>>>()?;

    eprintln!(
        "{:>12} in {}",
        "Proto files".bold().cyan(),
        proto_root.display()
    );
    for proto in &protos {
        eprintln!(
            "{:>12} - {}",
            "",
            proto.file_name().unwrap().to_string_lossy()
        );
    }

    let out = repo_root.join("rust/ommx/src");
    eprintln!(
        "{:>12} {}",
        "Writing".bold().cyan(),
        out.join("ommx.rs").display()
    );

    let mut cfg = Config::new();
    cfg.out_dir(&out).compile_protos(&protos, &[proto_root])?;

    std::process::Command::new("rustfmt")
        .arg(out.join("ommx.rs"))
        .status()?;

    Ok(())
}
