use anyhow::{bail, Result};
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
    let mut protos = protos;
    protos.sort();

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
        "{:>12} generated protobuf Rust modules under {}",
        "Writing".bold().cyan(),
        out.display()
    );

    let mut cfg = Config::new();
    cfg.type_attribute(".", "#[non_exhaustive]");
    cfg.field_attribute("SampleSet.feasible_unrelaxed", "#[deprecated]");
    cfg.btree_map([".ommx.v2"]);
    cfg.out_dir(&out).compile_protos(&protos, &[proto_root])?;

    let mut generated = glob(&format!("{}/ommx.*.rs", out.display()))?
        .map(|entry| -> Result<PathBuf> { Ok(entry?) })
        .collect::<Result<Vec<_>>>()?;
    generated.sort();
    for file in generated {
        eprintln!("{:>12} {}", "Formatting".bold().cyan(), file.display());
        let status = std::process::Command::new("rustfmt").arg(&file).status()?;
        if !status.success() {
            bail!(
                "rustfmt failed for {} with status {}",
                file.display(),
                status
            );
        }
    }

    Ok(())
}
