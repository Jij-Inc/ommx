use anyhow::Result;
use colored::Colorize;
use glob::glob;
use prost_build::Config;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let manifest_root: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let repo_root = manifest_root.join("../..").canonicalize()?;
    let proto_root = repo_root.join("proto");

    let protos = glob(&format!("{}/**/*.proto", proto_root.display()))?
        .map(|entry| -> Result<PathBuf> { Ok(entry?) })
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
    // FIXME: Get from prost
    let filename = "ommx.v1.rs";
    let out_file = out.join(filename);
    eprintln!("{:>12} {}", "Writing".bold().cyan(), out_file.display());

    let mut cfg = Config::new();
    cfg.type_attribute(".", "#[non_exhaustive]");
    cfg.field_attribute("SampleSet.feasible", "#[deprecated]");
    cfg.out_dir(&out).compile_protos(&protos, &[proto_root])?;

    std::process::Command::new("rustfmt")
        .arg(out_file)
        .status()?;

    Ok(())
}
