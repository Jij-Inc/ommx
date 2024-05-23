use anyhow::{Context, Result};
use colored::Colorize;
use ocipkg::ImageName;
use ommx::{artifact::Builder, random::random_lp};
use rand::SeedableRng;
use std::path::Path;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn main() -> Result<()> {
    let mut rng = rand_xoshiro::Xoshiro256StarStar::seed_from_u64(0);
    let lp = random_lp(&mut rng, 5, 7);

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
        "ghcr.io/Jij-Inc/ommx/random_lp_instance:{}",
        built_info::GIT_COMMIT_HASH.context("Cannot get commit hash of Git")?
    ))?;

    println!("{:>12} {}", "New Artifact".blue().bold(), image_name);
    let _artifact = Builder::new_archive(out.clone(), image_name)?
        .add_instance(lp, Default::default())?
        .build()?;
    println!("{:>12} {}", "Saved".green().bold(), out.display());
    Ok(())
}
