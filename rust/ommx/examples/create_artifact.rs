use anyhow::{Context, Result};
use colored::Colorize;
use ommx::{
    artifact::{ImageRef, InstanceAnnotations, LocalArtifactBuilder},
    random::random_deterministic,
    InstanceParameters,
};
use std::path::Path;
use url::Url;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let lp: ommx::Instance = random_deterministic(InstanceParameters::default_lp());
    let lp: ommx::v1::Instance = lp.into();

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

    let image_name = ImageRef::parse(&format!(
        "ghcr.io/jij-inc/ommx/random_lp_instance:{}",
        built_info::GIT_COMMIT_HASH_SHORT.context("Cannot get commit hash of Git")?
    ))?;

    println!("{:>12} {}", "New Artifact".blue().bold(), image_name);
    let mut annotations = InstanceAnnotations::default();
    annotations.set_title("random_lp".to_string());
    annotations.set_created(chrono::Local::now());

    let mut builder = LocalArtifactBuilder::new(image_name);
    builder.add_instance(lp, annotations)?;
    builder.add_source(&Url::parse("https://github.com/Jij-Inc/ommx")?);
    builder.add_annotation(
        "org.opencontainers.image.description",
        "Test artifact created by examples/create_artifact.rs",
    );
    let artifact = builder.build()?;
    artifact.save(&out)?;
    println!("{:>12} {}", "Saved".green().bold(), out.display());
    Ok(())
}
