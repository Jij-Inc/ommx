use anyhow::Result;
use colored::Colorize;
use ommx::{artifact::Builder, random::random_lp};
use rand::SeedableRng;
use std::path::Path;

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
    println!("{:>12} {}", "New Artifact".blue().bold(), out.display());
    let _artifact = Builder::new_archive_unnamed(out)?
        .add_instance(lp, Default::default())?
        .build()?;
    Ok(())
}
