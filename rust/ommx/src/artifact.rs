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
