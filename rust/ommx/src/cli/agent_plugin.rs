//! The CLI owns its distributable agent plugin independently of mathematical
//! objects and the Artifact Local Registry. Its layout matches JijModeling's
//! bundle; embedding also supports standalone binaries without a source tree.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const FILES: &[(&str, &str)] = &[
    (
        ".claude-plugin/marketplace.json",
        include_str!("../../agent_plugin/.claude-plugin/marketplace.json"),
    ),
    (
        "plugins/ommx/plugin.json",
        include_str!("../../agent_plugin/plugins/ommx/plugin.json"),
    ),
    (
        "plugins/ommx/.claude-plugin/plugin.json",
        include_str!("../../agent_plugin/plugins/ommx/.claude-plugin/plugin.json"),
    ),
    (
        "plugins/ommx/skills/ommx/SKILL.md",
        include_str!("../../agent_plugin/plugins/ommx/skills/ommx/SKILL.md"),
    ),
];

pub fn path() -> Result<PathBuf> {
    let cache = match std::env::var_os("OMMX_PLUGIN_CACHE_DIR") {
        Some(path) => PathBuf::from(path),
        None => directories::ProjectDirs::from("org", "ommx", "ommx")
            .context("Cannot locate the OMMX plugin cache: no home directory is available; set OMMX_PLUGIN_CACHE_DIR")?
            .cache_dir()
            .join("plugins"),
    };
    materialize(&cache)
}

fn bundle_files() -> Result<Vec<(&'static str, String)>> {
    FILES
        .iter()
        .map(|&(path, contents)| {
            let contents = if path.ends_with("/plugin.json") {
                let mut manifest: serde_json::Value =
                    serde_json::from_str(contents).with_context(|| {
                        format!("Failed to parse the bundled plugin manifest {path}")
                    })?;
                // The shared CLI is owned and versioned by the Rust SDK, including
                // when invoked through Python. Derive both manifest versions here
                // so version changes never need a second metadata update.
                manifest["version"] = env!("CARGO_PKG_VERSION").into();
                serde_json::to_string_pretty(&manifest)? + "\n"
            } else {
                contents.to_owned()
            };
            Ok((path, contents))
        })
        .collect()
}

fn materialize(cache: &Path) -> Result<PathBuf> {
    let files = bundle_files()?;
    // Include every relative path and its versioned contents in the identity.
    // Length prefixes make boundaries explicit across multi-file bundles.
    let mut hasher = Sha256::new();
    for (path, contents) in &files {
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update((contents.len() as u64).to_le_bytes());
        hasher.update(contents.as_bytes());
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let root = cache.join(digest);
    for (relative_path, contents) in files {
        let path = root.join(relative_path);
        let directory = path.parent().expect("all embedded paths have a parent");
        fs::create_dir_all(directory).with_context(|| {
            format!(
                "Failed to create the OMMX plugin cache at {}",
                directory.display()
            )
        })?;
        if fs::read(&path).ok().as_deref() != Some(contents.as_bytes()) {
            // Publish complete files atomically for concurrent invocations.
            // Deleted or altered cache entries are restored from the bundle.
            let mut file = tempfile::NamedTempFile::new_in(directory).with_context(|| {
                format!(
                    "Failed to stage the bundled plugin at {}",
                    directory.display()
                )
            })?;
            file.write_all(contents.as_bytes()).with_context(|| {
                format!("Failed to write the bundled plugin file {relative_path}")
            })?;
            file.persist(&path).with_context(|| {
                format!(
                    "Failed to publish the bundled plugin file at {}",
                    path.display()
                )
            })?;
        }
    }
    root.canonicalize().with_context(|| {
        format!(
            "Failed to resolve the OMMX plugin path at {}",
            root.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_complete_bundle(root: &Path) {
        for (relative_path, contents) in bundle_files().unwrap() {
            assert_eq!(
                fs::read_to_string(root.join(relative_path)).unwrap(),
                contents
            );
        }
    }

    #[test]
    fn materialization_is_complete_stable_and_repairs_every_cache_entry() {
        let cache = tempfile::tempdir().unwrap();
        let root = materialize(cache.path()).unwrap();
        assert!(root.is_absolute());
        assert_complete_bundle(&root);
        assert_eq!(materialize(cache.path()).unwrap(), root);
        for (relative_path, _) in FILES {
            let path = root.join(relative_path);
            fs::write(&path, "altered cache entry").unwrap();
            assert_eq!(materialize(cache.path()).unwrap(), root);
            assert_complete_bundle(&root);
            fs::remove_file(&path).unwrap();
            assert_eq!(materialize(cache.path()).unwrap(), root);
            assert_complete_bundle(&root);
        }
    }

    #[test]
    fn concurrent_materialization_publishes_the_same_complete_plugin() {
        let cache = tempfile::tempdir().unwrap();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| scope.spawn(|| materialize(cache.path()).unwrap()))
                .collect();
            let roots: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            assert!(roots.iter().all(|root| root == &roots[0]));
            assert_complete_bundle(&roots[0]);
        });
    }

    #[test]
    fn marketplace_sources_resolve_to_versioned_plugins() {
        let cache = tempfile::tempdir().unwrap();
        let root = materialize(cache.path()).unwrap();
        let marketplace: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join(".claude-plugin/marketplace.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(marketplace["name"], "ommx");
        let entry = &marketplace["plugins"][0];
        let plugin = root.join(entry["source"].as_str().unwrap());
        assert!(plugin.canonicalize().unwrap().starts_with(&root));
        for relative_path in ["plugin.json", ".claude-plugin/plugin.json"] {
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(plugin.join(relative_path)).unwrap()).unwrap();
            assert_eq!(manifest["name"], entry["name"]);
            assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
        }
        assert!(plugin.join("skills/ommx/SKILL.md").is_file());
    }

    #[test]
    fn invalid_cache_location_preserves_the_io_cause() {
        let cache = tempfile::tempdir().unwrap();
        let blocked = cache.path().join("file");
        fs::write(&blocked, "not a directory").unwrap();
        let error = materialize(&blocked).unwrap_err();
        assert!(format!("{error:#}").contains("Failed to create the OMMX plugin cache"));
        assert!(error.downcast_ref::<std::io::Error>().is_some());
    }
}
