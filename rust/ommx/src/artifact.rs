//! Manage messages as container
//!

mod annotations;
mod artifact;
mod builder;
mod config;
mod media_type;
pub use annotations::*;
pub use artifact::*;
pub use builder::*;
pub use config::*;
pub use media_type::*;

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Root directory for OMMX artifacts
pub fn data_dir() -> Result<PathBuf> {
    Ok(directories::ProjectDirs::from("org", "ommx", "ommx")
        .context("Failed to get project directories")?
        .data_dir()
        .to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::random_lp;
    use rand::SeedableRng;
    use std::path::Path;

    #[test]
    fn save_random_lp_as_archive() -> Result<()> {
        let mut rng = rand_xoshiro::Xoshiro256StarStar::seed_from_u64(0);
        let lp = random_lp(&mut rng, 5, 7);
        let root: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
        let out = root.join("random_lp.ommx");
        if out.exists() {
            std::fs::remove_file(&out)?;
        }
        let _artifact = Builder::new_archive_unnamed(out)?
            .add_instance(lp, Default::default())?
            .build()?;
        Ok(())
    }
}
