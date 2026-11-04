#![cfg(feature = "cli")]

use std::{path::PathBuf, process::Command};

#[test]
fn standalone_discovery_is_offline_and_reports_exit_codes() {
    let temporary = tempfile::tempdir().unwrap();
    let cache = temporary.path().join("plugin-cache-例");
    let registry = temporary.path().join("registry");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_ommx"))
            .args(args)
            .current_dir(temporary.path())
            .env("OMMX_PLUGIN_CACHE_DIR", &cache)
            .env("OMMX_LOCAL_REGISTRY_ROOT", &registry)
            .output()
            .unwrap()
    };
    assert!(run(&["plugin", "path", "--help"]).status.success());
    assert!(!cache.exists());
    let invalid = run(&["skill", "path", "../other"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    assert!(!cache.exists());

    for args in [
        vec!["plugin", "path"],
        vec!["plugin", "path", "ommx"],
        vec!["plugin", "marketplace", "path"],
        vec!["skill", "path"],
        vec!["skill", "path", "ommx"],
    ] {
        let result = run(&args);
        assert!(result.status.success(), "{:?}", result.stderr);
        assert!(result.stderr.is_empty());
        let stdout = String::from_utf8(result.stdout).unwrap();
        assert_eq!(stdout.lines().count(), 1);
        let path = PathBuf::from(stdout.trim_end());
        assert!(path.is_absolute());
        assert!(path.is_dir());
        assert!(path.starts_with(cache.canonicalize().unwrap()));
    }
    assert!(!registry.exists());

    std::fs::remove_dir_all(&cache).unwrap();
    std::fs::write(&cache, "not a directory").unwrap();
    let failed = run(&["plugin", "path"]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(failed.stdout.is_empty());
    assert!(String::from_utf8(failed.stderr)
        .unwrap()
        .contains("Failed to create the OMMX plugin cache"));
    assert!(!registry.exists());
}
