//! Regression test for `ommx login` on a fresh environment.
//!
//! Previously, `ommx login` propagated `StoredAuth::load_all`'s failure
//! and exited with `No valid auth info found` when none of Docker's,
//! podman's, or ocipkg's auth files existed — making it impossible to
//! bootstrap credentials via `ommx login` alone. The CLI now falls back
//! to an empty auth store in that case, so the command can proceed to
//! the registry challenge step.
//!
//! Run with: `cargo test -p ommx --test login_no_auth`

use std::path::PathBuf;
use std::process::Command;

#[test]
fn login_proceeds_without_existing_auth_files() {
    let home = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("login_no_auth_home");
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).expect("create empty HOME for test");

    let output = Command::new(env!("CARGO_BIN_EXE_ommx"))
        .args(["login", "https://ghcr.invalid", "-u", "user", "-p", "pass"])
        .env("HOME", &home)
        .env_remove("XDG_RUNTIME_DIR")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("spawn ommx binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("No valid auth info found"),
        "login must not bail out on the auth-loading step; stderr={stderr}",
    );

    // The bogus `ghcr.invalid` URL guarantees the command still fails
    // at the network step, which proves we got past `load_all`.
    assert!(
        !output.status.success(),
        "expected the bogus registry URL to fail at the network step; \
         status={:?} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        stderr,
    );
}
