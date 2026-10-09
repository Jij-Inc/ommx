//! The CLI owns distribution of coding-agent instructions, independently of
//! mathematical objects and the Artifact Local Registry. Embed the instructions
//! so both the standalone binary and Python wheels work without a source tree.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const SKILL: &str = include_str!("../../skills/ommx/SKILL.md");

pub fn path() -> Result<PathBuf> {
    let cache = match std::env::var_os("OMMX_SKILL_CACHE_DIR") {
        Some(path) => PathBuf::from(path),
        None => directories::ProjectDirs::from("org", "ommx", "ommx")
            .context("Cannot locate the OMMX skill cache: no home directory is available; set OMMX_SKILL_CACHE_DIR")?
            .cache_dir()
            .join("skills"),
    };
    materialize(&cache)
}

fn materialize(cache: &Path) -> Result<PathBuf> {
    // Content identity keeps different SDK installations independent, including
    // development builds whose instructions change without a version bump.
    let digest: String = Sha256::digest(SKILL.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let root = cache.join(digest);
    let directory = root.join("ommx");
    let path = directory.join("SKILL.md");
    fs::create_dir_all(&directory).with_context(|| {
        format!(
            "Failed to create the OMMX skill cache at {}",
            directory.display()
        )
    })?;
    if fs::read(&path).ok().as_deref() != Some(SKILL.as_bytes()) {
        // Publishing a complete file atomically also permits concurrent CLI
        // invocations and repairs a deleted or altered cache entry.
        let mut file = tempfile::NamedTempFile::new_in(&directory).with_context(|| {
            format!(
                "Failed to stage the bundled skill at {}",
                directory.display()
            )
        })?;
        file.write_all(SKILL.as_bytes())
            .context("Failed to write the bundled OMMX skill")?;
        file.persist(&path).with_context(|| {
            format!(
                "Failed to publish the bundled OMMX skill at {}",
                path.display()
            )
        })?;
    }
    root.canonicalize().with_context(|| {
        format!(
            "Failed to resolve the OMMX skill path at {}",
            root.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_is_complete_stable_and_repairs_cache_entries() {
        let cache = tempfile::tempdir().unwrap();
        let root = materialize(cache.path()).unwrap();
        assert!(root.is_absolute());
        let path = root.join("ommx/SKILL.md");
        assert_eq!(fs::read_to_string(&path).unwrap(), SKILL);
        assert_eq!(materialize(cache.path()).unwrap(), root);
        fs::write(&path, "altered cache entry").unwrap();
        assert_eq!(materialize(cache.path()).unwrap(), root);
        assert_eq!(fs::read_to_string(&path).unwrap(), SKILL);
        fs::remove_file(&path).unwrap();
        materialize(cache.path()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), SKILL);
    }

    #[test]
    fn concurrent_materialization_publishes_the_same_complete_skill() {
        let cache = tempfile::tempdir().unwrap();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| scope.spawn(|| materialize(cache.path()).unwrap()))
                .collect();
            let roots: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            assert!(roots.iter().all(|root| root == &roots[0]));
            assert_eq!(
                fs::read_to_string(roots[0].join("ommx/SKILL.md")).unwrap(),
                SKILL
            );
        });
    }

    #[test]
    fn invalid_cache_location_preserves_the_io_cause() {
        let cache = tempfile::tempdir().unwrap();
        let blocked = cache.path().join("file");
        fs::write(&blocked, "not a directory").unwrap();
        let error = materialize(&blocked).unwrap_err();
        assert!(format!("{error:#}").contains("Failed to create the OMMX skill cache"));
        assert!(error.downcast_ref::<std::io::Error>().is_some());
    }
}
