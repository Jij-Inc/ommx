//! The CLI owns its distributable agent plugin independently of mathematical
//! objects and the Artifact Local Registry. Its layout matches JijModeling's
//! bundle; embedding also supports standalone binaries without a source tree.

use anyhow::{ensure, Context, Result};
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
    // The path is a CLI value, so it must be representable as one UTF-8 line.
    validate_cache_path(cache)?;
    fs::create_dir_all(cache).with_context(|| {
        format!(
            "Failed to create the OMMX plugin cache at {}",
            cache.display()
        )
    })?;
    // A user-selected cache may itself be a symlink. Resolve that intentional
    // root once; generated directories below it must be real directories.
    let cache = cache.canonicalize().with_context(|| {
        format!(
            "Failed to resolve the OMMX plugin cache at {}",
            cache.display()
        )
    })?;
    validate_cache_path(&cache)?;
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
    ensure_cache_directory(&root)?;
    for (relative_path, contents) in files {
        let path = root.join(relative_path);
        let mut directory = root.clone();
        for component in Path::new(relative_path)
            .parent()
            .expect("all embedded paths have a parent")
            .components()
        {
            directory.push(component);
            ensure_cache_directory(&directory)?;
        }
        let regular_file = fs::symlink_metadata(&path)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink());
        if !regular_file || fs::read(&path).ok().as_deref() != Some(contents.as_bytes()) {
            // Publish complete files atomically for concurrent invocations.
            // Deleted or altered cache entries are restored from the bundle.
            let mut file = tempfile::NamedTempFile::new_in(&directory).with_context(|| {
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

fn validate_cache_path(path: &Path) -> Result<()> {
    let path = path.to_str().context(
        "The OMMX plugin cache path must be UTF-8; set OMMX_PLUGIN_CACHE_DIR to a UTF-8 directory",
    )?;
    ensure!(
        !path.contains([
            '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
            '\u{2029}',
        ]),
        "The OMMX plugin cache path must not contain line breaks; set OMMX_PLUGIN_CACHE_DIR to a single-line directory"
    );
    Ok(())
}

fn ensure_cache_directory(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).with_context(|| {
                format!(
                    "Failed to inspect the OMMX plugin cache at {}",
                    path.display()
                )
            })?;
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Refusing to use a non-directory or symlink in the generated OMMX plugin cache at {}; remove this cache entry and retry",
                path.display()
            );
            Ok(())
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "Failed to create the OMMX plugin cache at {}",
                path.display()
            )
        }),
    }
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

    #[test]
    fn cache_paths_with_line_breaks_are_rejected_before_writing() {
        let temporary = tempfile::tempdir().unwrap();
        for separator in [
            '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
            '\u{2029}',
        ] {
            let cache = temporary.path().join(format!("cache{separator}break"));
            let error = materialize(&cache).unwrap_err();
            assert!(format!("{error:#}").contains("must not contain line breaks"));
            assert!(!cache.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_cache_paths_are_rejected_before_writing() {
        use std::os::unix::ffi::OsStringExt;

        let temporary = tempfile::tempdir().unwrap();
        let cache = temporary
            .path()
            .join(std::ffi::OsString::from_vec(vec![0xff]));
        let error = materialize(&cache).unwrap_err();
        assert!(format!("{error:#}").contains("must be UTF-8"));
        assert!(!cache.exists());
    }

    #[cfg(unix)]
    #[test]
    fn generated_directory_symlinks_cannot_write_outside_the_cache() {
        use std::os::unix::fs::symlink;

        for relative in ["", "plugins/ommx", "plugins/ommx/skills/ommx"] {
            let temporary = tempfile::tempdir().unwrap();
            let cache = temporary.path().join("cache");
            let root = materialize(&cache).unwrap();
            let outside = temporary.path().join("outside");
            fs::create_dir(&outside).unwrap();
            let victim = outside.join("SKILL.md");
            fs::write(&victim, "outside cache content").unwrap();
            let entry = if relative.is_empty() {
                root
            } else {
                root.join(relative)
            };
            fs::remove_dir_all(&entry).unwrap();
            symlink(&outside, &entry).unwrap();

            let error = materialize(&cache).unwrap_err();
            assert!(format!("{error:#}").contains("Refusing to use"));
            assert_eq!(
                fs::read_to_string(&victim).unwrap(),
                "outside cache content"
            );
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        }
    }

    #[cfg(unix)]
    #[test]
    fn file_symlinks_are_replaced_without_modifying_the_target() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let cache = temporary.path().join("cache");
        let root = materialize(&cache).unwrap();
        let path = root.join("plugins/ommx/skills/ommx/SKILL.md");
        let expected = fs::read(&path).unwrap();
        let outside = temporary.path().join("outside.md");
        fs::write(&outside, &expected).unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&outside, &path).unwrap();

        assert_eq!(materialize(&cache).unwrap(), root);
        assert!(!fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&outside).unwrap(), expected);
        assert_complete_bundle(&root);
    }

    #[cfg(unix)]
    #[test]
    fn user_selected_cache_symlinks_are_resolved() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let cache = temporary.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let alias = temporary.path().join("alias");
        symlink(&cache, &alias).unwrap();
        let root = materialize(&alias).unwrap();
        assert!(root.starts_with(cache.canonicalize().unwrap()));
        assert_complete_bundle(&root);
    }

    #[cfg(unix)]
    #[test]
    fn cache_symlinks_cannot_hide_line_breaks_in_the_returned_path() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let cache = temporary.path().join("cache\nbreak");
        fs::create_dir(&cache).unwrap();
        let alias = temporary.path().join("alias");
        symlink(&cache, &alias).unwrap();
        let error = materialize(&alias).unwrap_err();
        assert!(format!("{error:#}").contains("must not contain line breaks"));
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }
}
