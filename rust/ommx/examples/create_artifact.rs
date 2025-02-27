use anyhow::{Context, Result};
use colored::Colorize;
use ocipkg::ImageName;
use ommx::{
    artifact::{Builder, InstanceAnnotations},
    random::{random_deterministic, FunctionParameters, InstanceParameters},
};
use std::path::Path;
use url::Url;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let linear_parameter = FunctionParameters {
        num_terms: 5,
        max_degree: 1,
        max_id: 10,
    };
    let lp = random_deterministic(InstanceParameters {
        num_constraints: 7,
        objective: linear_parameter,
        constraint: linear_parameter,
        kinds: vec![ommx::v1::decision_variable::Kind::Continuous],
    });

    // "data" directory is at the root of the repository
    let manifest_root: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let repo_root = manifest_root.join("../..").canonicalize()?;
    let data_root = repo_root.join("data");
    if !data_root.exists() {
        println!("{:>12} {}", "Created".blue().bold(), data_root.display());
        std::fs::create_dir(&data_root)?;
    }

    let out = data_root.join("random_lp_instance.ommx");
    if out.exists() {
        println!("{:>12} {}", "Removing".red().bold(), out.display());
        std::fs::remove_file(&out)?;
    }

    let image_name = ImageName::parse(&format!(
        "ghcr.io/jij-inc/ommx/random_lp_instance:{}",
        built_info::GIT_COMMIT_HASH_SHORT.context("Cannot get commit hash of Git")?
    ))?;

    println!("{:>12} {}", "New Artifact".blue().bold(), image_name);
    let mut annotations = InstanceAnnotations::default();
    annotations.set_title("random_lp".to_string());
    annotations.set_created(chrono::Local::now());

    let mut builder = Builder::new_archive(out.clone(), image_name)?;
    builder.add_instance(lp, annotations)?;
    builder.add_source(&Url::parse("https://github.com/Jij-Inc/ommx")?);
    builder.add_description("Test artifact created by examples/create_artifact.rs".to_string());
    let _artifact = builder.build()?;
    println!("{:>12} {}", "Saved".green().bold(), out.display());
    Ok(())
}
