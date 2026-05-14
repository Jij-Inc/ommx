//! Reproduces a known limitation of `ommx login` on 2.x: the command
//! delegates to `ocipkg::distribution::StoredAuth::load_all`, which errors
//! with `No valid auth info found` when none of Docker's, podman's, or
//! ocipkg's auth files exist. As a result, `ommx login` cannot bootstrap
//! credentials on a fresh machine and the user must run `docker login`
//! (or similar) first.
//!
//! This test pins that current behavior on dev-2.x. In v3 the `login`
//! subcommand is being replaced by docker-credential helpers, so this
//! limitation will no longer be relevant.
//!
//! Run with: `cargo test -p ommx --test login_no_auth`

use std::path::PathBuf;
use std::process::Command;

#[test]
fn login_fails_when_no_auth_file_exists() {
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

    assert!(
        !output.status.success(),
        "ommx login should fail on a fresh environment; status={:?} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No valid auth info found"),
        "expected 'No valid auth info found' in stderr, got: {stderr}",
    );
}
