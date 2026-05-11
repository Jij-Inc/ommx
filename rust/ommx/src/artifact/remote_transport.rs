//! Sync wrapper around an async OCI Distribution client.
//!
//! v3 replaces `ocipkg`'s `RemoteBuilder` with [`oci-client`] (the ORAS
//! project's actively-maintained successor to `oci-distribution`) as the
//! OCI Distribution transport. `oci-client` is async-only; this module
//! exposes a sync surface by owning a private `tokio` current-thread
//! runtime and dispatching every call through `Runtime::block_on`. The
//! runtime is created lazily on the first push and torn down with the
//! [`RemoteTransport`] value.
//!
//! Public callers see no `async`, no `tokio` re-export, and no runtime
//! lifetime — the wrapper is the only seam where async crosses into the
//! rest of the SDK. Later milestones expand the async boundary outward
//! (see `ARTIFACT_V3.md` §12.3 Step B) until pyo3-async runtimes expose
//! `await` on the Python side; until then this `block_on` wrapper is the
//! single point that needs to change.
//!
//! Pull / list / probe surface is intentionally not implemented yet —
//! Step B is push-only. The corresponding read paths still go through
//! `ocipkg` in `local_registry::import::remote`.
//!
//! Credentials are resolved by [`resolve_auth`] in a three-tier chain:
//! `OMMX_BASIC_AUTH_*` env override → `~/.docker/config.json`
//! (+ credential helpers via [`docker_credential`]) → anonymous. OMMX
//! does not own its own credential store; the docker tooling tier is
//! how `docker login` / `gcloud auth configure-docker` / `aws ecr
//! get-login-password` already surface registry credentials to the
//! container ecosystem.
//!
//! [`oci-client`]: https://github.com/oras-project/rust-oci-client

use anyhow::Context;
use docker_credential::{CredentialRetrievalError, DockerCredential};
use http::HeaderValue;
use oci_client::{
    client::{ClientConfig, ClientProtocol},
    secrets::RegistryAuth,
    Client, Reference, RegistryOperation,
};
use std::env;
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

/// Sync wrapper around [`oci_client::Client`].
pub(crate) struct RemoteTransport {
    runtime: Runtime,
    client: Client,
    auth: RegistryAuth,
}

impl RemoteTransport {
    /// Build a transport configured for the given `image_name`'s registry.
    /// Credentials are resolved by [`resolve_auth`]: first the
    /// `OMMX_BASIC_AUTH_*` env vars (explicit override), then
    /// `~/.docker/config.json` and docker credential helpers, then
    /// anonymous as a final fallback. Anonymous is sufficient for
    /// unauthenticated public reads but will fail at push time on
    /// registries that require auth.
    pub(crate) fn new(image_name: &ocipkg::ImageName) -> crate::Result<Self> {
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to build tokio runtime for OCI remote transport")?;

        let config = ClientConfig {
            protocol: protocol_for(&image_name.hostname),
            ..ClientConfig::default()
        };
        let client = Client::new(config);
        let auth = resolve_auth(&image_name.hostname);

        Ok(Self {
            runtime,
            client,
            auth,
        })
    }

    /// Authenticate to the registry once before issuing other requests.
    /// `oci-client` defers auth challenges until the first request, but
    /// for push flows we want bearer-token negotiation to happen up
    /// front so that errors surface with the operation that triggered
    /// them rather than buried inside a blob upload.
    pub(crate) fn auth(&self, image_name: &ocipkg::ImageName) -> crate::Result<()> {
        let reference = to_reference(image_name)?;
        self.runtime
            .block_on(
                self.client
                    .auth(&reference, &self.auth, RegistryOperation::Push),
            )
            .with_context(|| format!("Failed to authenticate against {reference}"))?;
        Ok(())
    }

    /// Push a single blob to the registry. The caller passes the
    /// pre-computed digest (which the SQLite Local Registry already
    /// stores as the BlobStore key), so the registry-side digest can be
    /// validated without re-hashing.
    pub(crate) fn push_blob(
        &self,
        image_name: &ocipkg::ImageName,
        digest: &str,
        bytes: &[u8],
    ) -> crate::Result<()> {
        let reference = to_reference(image_name)?;
        self.runtime
            .block_on(self.client.push_blob(&reference, bytes.to_vec(), digest))
            .with_context(|| format!("Failed to push blob {digest} to {reference}"))?;
        Ok(())
    }

    /// Push the manifest bytes verbatim with the caller-supplied OCI
    /// `Content-Type`. `oci_client::Client::push_manifest_raw` skips
    /// the typed `OciManifest` round-trip, so the digest stored locally
    /// and the digest the registry computes agree byte-for-byte.
    pub(crate) fn push_manifest_bytes(
        &self,
        image_name: &ocipkg::ImageName,
        manifest_bytes: &[u8],
        content_type: &str,
    ) -> crate::Result<()> {
        let reference = to_reference(image_name)?;
        let header: HeaderValue = content_type
            .parse()
            .with_context(|| format!("Invalid Content-Type {content_type}"))?;
        self.runtime
            .block_on(
                self.client
                    .push_manifest_raw(&reference, manifest_bytes.to_vec(), header),
            )
            .with_context(|| format!("Failed to push manifest to {reference}"))?;
        Ok(())
    }
}

/// Build a [`Reference`] from `ocipkg`'s `ImageName`. The two crates pull
/// in different versions of `oci-spec`, so the canonical interchange is
/// the stringified image name rather than a typed conversion.
fn to_reference(image_name: &ocipkg::ImageName) -> crate::Result<Reference> {
    let raw = image_name.to_string();
    raw.parse::<Reference>()
        .with_context(|| format!("Invalid OCI image reference: {raw}"))
        .map_err(Into::into)
}

/// `oci-client` defaults to HTTPS; `localhost` registries (used in tests
/// and Docker-in-Docker setups) are HTTP. Match the heuristic ocipkg has
/// historically used so the transition is transparent.
fn protocol_for(hostname: &str) -> ClientProtocol {
    if hostname == "localhost" || hostname.starts_with("127.") || hostname.starts_with("::1") {
        ClientProtocol::Http
    } else {
        ClientProtocol::Https
    }
}

/// Three-tier credential resolution for the target registry host:
///
/// 1. `OMMX_BASIC_AUTH_*` env vars — explicit override, intended for CI
///    and unattended runs.
/// 2. `~/.docker/config.json` (honouring `$DOCKER_CONFIG`) — matches the
///    docker / podman / oras tooling UX. Credential helpers
///    (`docker-credential-gcloud`, `docker-credential-ecr-login`, …) are
///    invoked transparently by [`docker_credential`].
/// 3. Anonymous — anonymous push will be rejected by the registry, but
///    unauthenticated public reads succeed.
///
/// The env-var override wins over docker config so that a `docker login`
/// session on the workstation can be deliberately bypassed in CI without
/// having to log out.
fn resolve_auth(hostname: &str) -> RegistryAuth {
    if let Some(auth) = auth_from_env(hostname) {
        return auth;
    }
    if let Some(auth) = auth_from_docker_config(hostname) {
        return auth;
    }
    tracing::debug!("No credentials resolved for {hostname}; using anonymous auth");
    RegistryAuth::Anonymous
}

fn auth_from_env(hostname: &str) -> Option<RegistryAuth> {
    let domain = env::var("OMMX_BASIC_AUTH_DOMAIN").ok()?;
    if domain != hostname {
        tracing::debug!(
            "OMMX_BASIC_AUTH_DOMAIN={domain} does not match target host {hostname}; \
             falling through to docker config"
        );
        return None;
    }
    let username = env::var("OMMX_BASIC_AUTH_USERNAME").ok()?;
    let password = env::var("OMMX_BASIC_AUTH_PASSWORD").ok()?;
    tracing::info!("Using OMMX_BASIC_AUTH credentials for {hostname} (user {username})");
    Some(RegistryAuth::Basic(username, password))
}

/// Look the hostname up in `~/.docker/config.json` (or `$DOCKER_CONFIG`).
/// Returns `None` when there is no config file, no entry for the host,
/// or a helper failure — the caller falls through to anonymous auth in
/// those cases, matching how `docker push` would surface the same
/// situation as a 401 from the registry rather than a local error.
///
/// Docker's `IdentityToken` is mapped to `RegistryAuth::Bearer`. This
/// matches what `oras` / `crane` do: identity tokens are short-lived
/// bearer tokens, not OAuth2 refresh tokens, despite the historical
/// naming.
fn auth_from_docker_config(hostname: &str) -> Option<RegistryAuth> {
    classify_docker_credential(hostname, docker_credential::get_credential(hostname))
}

/// Map a `docker_credential` result into [`RegistryAuth`], emitting
/// tracing events at the same granularity as the env-var path.
///
/// Extracted as a separate function so unit tests can exercise the
/// classification logic without touching `~/.docker/config.json`.
fn classify_docker_credential(
    hostname: &str,
    result: std::result::Result<DockerCredential, CredentialRetrievalError>,
) -> Option<RegistryAuth> {
    match result {
        Ok(DockerCredential::UsernamePassword(username, password)) => {
            tracing::info!("Using docker config Basic auth for {hostname} (user {username})");
            Some(RegistryAuth::Basic(username, password))
        }
        Ok(DockerCredential::IdentityToken(token)) => {
            tracing::info!("Using docker config identity token for {hostname}");
            Some(RegistryAuth::Bearer(token))
        }
        Err(
            CredentialRetrievalError::ConfigNotFound
            | CredentialRetrievalError::NoCredentialConfigured,
        ) => None,
        Err(e) => {
            tracing::warn!(
                "Failed to read docker credential for {hostname}: {e}; falling through to anonymous"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_basic(auth: Option<RegistryAuth>, expected_user: &str, expected_pass: &str) {
        match auth {
            Some(RegistryAuth::Basic(u, p)) => {
                assert_eq!(u, expected_user);
                assert_eq!(p, expected_pass);
            }
            other => panic!("expected Basic({expected_user}, _), got {other:?}"),
        }
    }

    fn assert_bearer(auth: Option<RegistryAuth>, expected_token: &str) {
        match auth {
            Some(RegistryAuth::Bearer(t)) => assert_eq!(t, expected_token),
            other => panic!("expected Bearer(_), got {other:?}"),
        }
    }

    /// `UsernamePassword` from docker config → `RegistryAuth::Basic`.
    /// Covers the typical `docker login` + `auths.{host}.auth` case as
    /// well as helper output like `docker-credential-gcloud` returning
    /// `oauth2accesstoken` / token.
    #[test]
    fn classify_username_password_to_basic() {
        let auth = classify_docker_credential(
            "ghcr.io",
            Ok(DockerCredential::UsernamePassword(
                "alice".to_string(),
                "secret".to_string(),
            )),
        );
        assert_basic(auth, "alice", "secret");
    }

    /// `IdentityToken` from docker config → `RegistryAuth::Bearer`. The
    /// docker JSON schema records this field for registries that return
    /// an identity token from the `/v2/token` endpoint.
    #[test]
    fn classify_identity_token_to_bearer() {
        let auth = classify_docker_credential(
            "ghcr.io",
            Ok(DockerCredential::IdentityToken("tok-123".to_string())),
        );
        assert_bearer(auth, "tok-123");
    }

    /// "No docker config on disk" and "host not in docker config" must
    /// both fall through to `None` so that `resolve_auth` can land on
    /// anonymous — the registry, not OMMX, decides what is missing.
    #[test]
    fn classify_missing_credentials_falls_through() {
        for err in [
            CredentialRetrievalError::ConfigNotFound,
            CredentialRetrievalError::NoCredentialConfigured,
        ] {
            assert!(classify_docker_credential("ghcr.io", Err(err)).is_none());
        }
    }

    /// Helper / decoding failures are also non-fatal: warn and fall
    /// through. The user sees the registry's own 401 rather than a
    /// crash from a broken `docker-credential-*` binary.
    #[test]
    fn classify_helper_failure_falls_through() {
        let auth = classify_docker_credential(
            "ghcr.io",
            Err(CredentialRetrievalError::HelperFailure {
                helper: "docker-credential-broken".to_string(),
                stdout: String::new(),
                stderr: "boom".to_string(),
            }),
        );
        assert!(auth.is_none());
    }
}
