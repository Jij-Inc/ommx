//! Sync wrapper around an async OCI Distribution client.
//!
//! v3 talks to OCI Distribution registries through [`oci-client`] (the ORAS
//! project's actively-maintained successor to `oci-distribution`) as the
//! OCI Distribution transport. `oci-client` is async-only; this module
//! exposes a sync surface by owning a private `tokio` current-thread
//! runtime and dispatching every call through `Runtime::block_on`. The
//! runtime is created lazily on the first push and torn down with the
//! [`RemoteTransport`] value.
//!
//! Public callers see no `async`, no `tokio` re-export, and no runtime
//! lifetime — the wrapper is the only seam where async crosses into the
//! rest of the SDK. Later refinements may expand the async boundary
//! outward until pyo3-async-runtimes exposes `await` on the Python
//! side; until then this `block_on` wrapper is the single point that
//! needs to change.
//!
//! Blob transfers run with four concurrent operations by default. Set
//! `OMMX_ARTIFACT_TRANSFER_CONCURRENCY` to a positive integer to override
//! that conservative limit for both push and pull.
//!
//! Both push and pull surfaces are implemented here.
//! `pull_manifest_raw` returns the manifest body verbatim so the
//! digest the registry computes and the digest we store locally agree
//! byte-for-byte; `pull_blob_to_vec` collects a blob into a `Vec<u8>`
//! over `oci-client`'s streaming reader. Layer-blob streaming straight
//! into the Local Registry is a future refinement once that
//! store grows an `AsyncWrite`-compatible put path.
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
use futures_util::{stream, StreamExt, TryStreamExt};
use http::HeaderValue;
use oci_client::{
    client::{ClientConfig, ClientProtocol},
    errors::DigestError,
    secrets::RegistryAuth,
    Client, Reference, RegistryOperation,
};
use std::{env, io, num::NonZeroUsize};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

const ARTIFACT_TRANSFER_CONCURRENCY_ENV: &str = "OMMX_ARTIFACT_TRANSFER_CONCURRENCY";
const DEFAULT_ARTIFACT_TRANSFER_CONCURRENCY: usize = 4;

/// Resolve the shared push/pull blob-transfer concurrency.
pub fn blob_transfer_concurrency() -> crate::Result<NonZeroUsize> {
    match env::var(ARTIFACT_TRANSFER_CONCURRENCY_ENV) {
        Ok(value) => {
            let concurrency = parse_blob_transfer_concurrency(Some(&value))?;
            tracing::debug!(
                concurrency = concurrency.get(),
                variable = ARTIFACT_TRANSFER_CONCURRENCY_ENV,
                "Using configured Artifact transfer concurrency"
            );
            Ok(concurrency)
        }
        Err(env::VarError::NotPresent) => parse_blob_transfer_concurrency(None),
        Err(env::VarError::NotUnicode(_)) => anyhow::bail!(
            "{ARTIFACT_TRANSFER_CONCURRENCY_ENV} must be a positive integer encoded as UTF-8"
        ),
    }
}

fn parse_blob_transfer_concurrency(value: Option<&str>) -> crate::Result<NonZeroUsize> {
    match value {
        Some(value) => value.parse::<NonZeroUsize>().with_context(|| {
            format!("{ARTIFACT_TRANSFER_CONCURRENCY_ENV} must be a positive integer, got {value:?}")
        }),
        None => Ok(NonZeroUsize::new(DEFAULT_ARTIFACT_TRANSFER_CONCURRENCY)
            .expect("default Artifact transfer concurrency must be positive")),
    }
}

pub async fn bounded_map<I, T, O, F, Fut>(
    items: I,
    concurrency: NonZeroUsize,
    f: F,
) -> crate::Result<Vec<O>>
where
    I: IntoIterator<Item = T>,
    F: FnMut(T) -> Fut,
    Fut: std::future::Future<Output = crate::Result<O>>,
{
    stream::iter(items)
        .map(f)
        .buffer_unordered(concurrency.get())
        .try_collect()
        .await
}

/// Sync wrapper around [`oci_client::Client`].
pub struct RemoteTransport {
    runtime: Runtime,
    client: Client,
    auth: RegistryAuth,
}

/// Invalid explicit credentials supplied through the OMMX environment override.
///
/// This marker stays private to the remote implementation. The Artifact error
/// boundary recognizes it by type and exposes it as
/// `RemoteArtifactError::Authentication` without matching its message.
#[derive(Debug, thiserror::Error)]
#[error(
    "OMMX_BASIC_AUTH_DOMAIN={domain} is set (matches target {domain}), \
     but OMMX_BASIC_AUTH_USERNAME={username_state} and \
     OMMX_BASIC_AUTH_PASSWORD={password_state}. Both variables are required \
     for the env-var auth override; unset OMMX_BASIC_AUTH_DOMAIN to fall back \
     to ~/.docker/config.json instead."
)]
pub struct InvalidAuthenticationConfiguration {
    domain: String,
    username_state: &'static str,
    password_state: &'static str,
}

/// A registry response that contradicts the pulled Artifact manifest.
///
/// Transport errors from `oci-client` remain separate. This marker lets the
/// Artifact boundary classify response-shape violations as an invalid remote
/// Artifact without relying on rendered error text.
#[derive(Debug, thiserror::Error)]
pub enum InvalidRemoteResponse {
    #[error(
        "Registry reported Content-Length {content_length} for blob {digest}, \
         but the manifest descriptor declares size {expected_size}"
    )]
    ContentLengthMismatch {
        digest: String,
        content_length: u64,
        expected_size: u64,
    },
    #[error("Pulled blob size overflowed u64 for {digest}")]
    SizeOverflow { digest: String },
    #[error(
        "Pulled blob bytes for {digest} exceed declared size {expected_size}; \
         the registry served {actual_size} bytes"
    )]
    BlobTooLarge {
        digest: String,
        expected_size: u64,
        actual_size: u64,
    },
    #[error("Blob digest verification failed for {digest}")]
    BlobDigestMismatch {
        digest: String,
        #[source]
        source: io::Error,
    },
}

/// I/O failure while consuming a response body from the remote registry.
///
/// This marker is distinct from local registry I/O. The Artifact error
/// boundary recognizes it by type and exposes it as
/// `RemoteArtifactError::Transport`.
#[derive(Debug, thiserror::Error)]
#[error("Remote blob stream failed for {digest}")]
pub struct RemoteTransportFailure {
    digest: String,
    #[source]
    source: io::Error,
}

fn blob_stream_error(digest: &str, source: io::Error) -> crate::Error {
    let is_digest_error = error_contains_digest_error(&source);

    if is_digest_error {
        crate::error!(InvalidRemoteResponse::BlobDigestMismatch {
            digest: digest.to_owned(),
            source,
        })
    } else {
        crate::error!(RemoteTransportFailure {
            digest: digest.to_owned(),
            source,
        })
    }
}

fn error_contains_digest_error(error: &(dyn std::error::Error + 'static)) -> bool {
    if error.downcast_ref::<DigestError>().is_some() {
        return true;
    }
    if let Some(io_error) = error.downcast_ref::<io::Error>() {
        // `std::io::Error::source()` delegates to the wrapped error's source,
        // so a `DigestError` passed to `io::Error::other` must be inspected via
        // `get_ref()` to retain the wrapper itself in this type-based check.
        if io_error
            .get_ref()
            .is_some_and(|inner| error_contains_digest_error(inner))
        {
            return true;
        }
    }
    error.source().is_some_and(error_contains_digest_error)
}

impl RemoteTransport {
    /// Build a transport configured for the given `image_name`'s registry.
    /// Credentials are resolved by [`resolve_auth`]: first the
    /// `OMMX_BASIC_AUTH_*` env vars (explicit override), then
    /// `~/.docker/config.json` and docker credential helpers, then
    /// anonymous as a final fallback. Anonymous is sufficient for
    /// unauthenticated public reads but will fail at push time on
    /// registries that require auth.
    pub fn new(image_name: &crate::artifact::ImageRef) -> crate::Result<Self> {
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to build tokio runtime for OCI remote transport")?;

        let config = ClientConfig {
            protocol: protocol_for(image_name.registry()),
            ..ClientConfig::default()
        };
        let client = Client::new(config);
        let auth = resolve_auth(image_name.registry())?;

        Ok(Self {
            runtime,
            client,
            auth,
        })
    }

    /// Authenticate to the registry once before issuing other requests.
    /// `oci-client` defers auth challenges until the first request, but
    /// for push / pull flows we want bearer-token negotiation to happen
    /// up front so that errors surface with the operation that triggered
    /// them rather than buried inside a blob transfer. Pass the
    /// [`RegistryOperation`] matching the next call (`Push` vs `Pull`);
    /// registries scope bearer tokens by operation.
    pub fn auth_for(
        &self,
        image_name: &crate::artifact::ImageRef,
        operation: RegistryOperation,
    ) -> crate::Result<()> {
        let reference = to_reference(image_name);
        self.runtime
            .block_on(self.client.auth(reference, &self.auth, operation))
            .with_context(|| format!("Failed to authenticate against {reference}"))?;
        Ok(())
    }

    /// Convenience: authenticate for a `Push` request. Most existing
    /// call sites push; the explicit form is [`Self::auth_for`].
    pub fn auth(&self, image_name: &crate::artifact::ImageRef) -> crate::Result<()> {
        self.auth_for(image_name, RegistryOperation::Push)
    }

    /// Async blob existence check used by the bounded push pipeline.
    pub async fn blob_exists_async(
        &self,
        image_name: &crate::artifact::ImageRef,
        digest: &str,
    ) -> crate::Result<bool> {
        let reference = to_reference(image_name);
        self.client
            .blob_exists(reference, digest)
            .await
            .with_context(|| format!("Failed to check blob {digest} in {reference}"))
    }

    /// Async blob upload used by the bounded push pipeline.
    pub async fn push_blob_async(
        &self,
        image_name: &crate::artifact::ImageRef,
        digest: &str,
        bytes: Vec<u8>,
    ) -> crate::Result<()> {
        let reference = to_reference(image_name);
        self.client
            .push_blob(reference, bytes, digest)
            .await
            .with_context(|| format!("Failed to push blob {digest} to {reference}"))?;
        Ok(())
    }

    /// Push the manifest bytes verbatim with the caller-supplied OCI
    /// `Content-Type`. `oci_client::Client::push_manifest_raw` skips
    /// the typed `OciManifest` round-trip, so the digest stored locally
    /// and the digest the registry computes agree byte-for-byte.
    /// `manifest_bytes` is moved through into `push_manifest_raw` to
    /// avoid cloning a manifest the caller already owns.
    pub fn push_manifest_bytes(
        &self,
        image_name: &crate::artifact::ImageRef,
        manifest_bytes: Vec<u8>,
        content_type: &str,
    ) -> crate::Result<()> {
        let reference = to_reference(image_name);
        let header: HeaderValue = content_type
            .parse()
            .with_context(|| format!("Invalid Content-Type {content_type}"))?;
        self.runtime
            .block_on(
                self.client
                    .push_manifest_raw(reference, manifest_bytes, header),
            )
            .with_context(|| format!("Failed to push manifest to {reference}"))?;
        Ok(())
    }

    /// Pull the manifest for `image_name` verbatim. Returns the raw
    /// bytes (so the digest the registry computes and the digest we
    /// store locally agree byte-for-byte) alongside the digest string
    /// reported by the registry. `accepted_media_types` is forwarded
    /// to the `Accept` header — pass the OMMX image-manifest media
    /// type for a manifest pull.
    pub fn pull_manifest_raw(
        &self,
        image_name: &crate::artifact::ImageRef,
        accepted_media_types: &[&str],
    ) -> crate::Result<(Vec<u8>, String)> {
        let reference = to_reference(image_name);
        let (bytes, digest) = self
            .runtime
            .block_on(
                self.client
                    .pull_manifest_raw(reference, &self.auth, accepted_media_types),
            )
            .with_context(|| format!("Failed to pull manifest from {reference}"))?;
        Ok((bytes.to_vec(), digest))
    }

    /// Pull one blob into memory for the bounded download pipeline.
    pub async fn pull_blob_to_vec_async(
        &self,
        image_name: &crate::artifact::ImageRef,
        digest: &str,
        expected_size: u64,
    ) -> crate::Result<Vec<u8>> {
        let reference = to_reference(image_name);
        let resp = self
            .client
            .pull_blob_stream(reference, digest)
            .await
            .with_context(|| format!("Failed to pull blob {digest} from {reference}"))?;
        if let Some(content_length) = resp.content_length {
            if content_length != expected_size {
                return Err(crate::error!(
                    InvalidRemoteResponse::ContentLengthMismatch {
                        digest: digest.to_owned(),
                        content_length,
                        expected_size,
                    }
                ));
            }
        }
        let prealloc = expected_size.min(BLOB_PREALLOC_CAP_BYTES) as usize;
        let mut buf = Vec::with_capacity(prealloc);
        let mut accumulated: u64 = 0;
        let mut stream = resp.stream;
        while let Some(chunk) = stream
            .try_next()
            .await
            .map_err(|source| blob_stream_error(digest, source))?
        {
            accumulated = accumulated.checked_add(chunk.len() as u64).ok_or_else(|| {
                crate::error!(InvalidRemoteResponse::SizeOverflow {
                    digest: digest.to_owned(),
                })
            })?;
            if accumulated > expected_size {
                return Err(crate::error!(InvalidRemoteResponse::BlobTooLarge {
                    digest: digest.to_owned(),
                    expected_size,
                    actual_size: accumulated,
                }));
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    }

    pub fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

/// Upper bound on `Vec::with_capacity` for blob downloads. Typical
/// OMMX layer blobs (problem instances, solutions) are well under
/// this, so legitimate traffic never hits the cap; a hostile manifest
/// that claims a multi-gigabyte size is contained to a single 256 MiB
/// pre-allocation rather than allocating from the claim directly.
/// The buffer can still grow past the cap if the actual stream
/// produces more bytes, at the cost of one or two reallocations.
const BLOB_PREALLOC_CAP_BYTES: u64 = 256 * 1024 * 1024;

/// Borrow the inner `oci_client::Reference` from an [`ImageRef`].
/// `oci_client::Reference` is `pub use`d straight from
/// `oci_spec::distribution::Reference`, which is the same type
/// [`ImageRef`] wraps, so no reparse is required —
/// [`ImageRef::as_inner`] hands out the borrow.
fn to_reference(image_name: &crate::artifact::ImageRef) -> &Reference {
    image_name.as_inner()
}

/// `oci-client` defaults to HTTPS; `localhost` registries (used in tests
/// and Docker-in-Docker setups) are HTTP. Apply the same `localhost`
/// heuristic the docker tooling uses so unauthenticated local pushes
/// don't require a custom client config.
///
/// `registry` is the joined `host[:port]` form straight out of
/// [`ImageRef::registry`]. The host portion is parsed inline rather
/// than via a dedicated accessor on [`ImageRef`] — it's a heuristic
/// local to this transport, not a v3 concept worth a method.
fn protocol_for(registry: &str) -> ClientProtocol {
    let host = registry
        .split_once(':')
        .map(|(host, _)| host)
        .unwrap_or(registry);
    if host == "localhost" || host.starts_with("127.") || host.starts_with("::1") {
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
fn resolve_auth(registry_key: &str) -> crate::Result<RegistryAuth> {
    if let Some(auth) = auth_from_env(registry_key)? {
        return Ok(auth);
    }
    if let Some(auth) = auth_from_docker_config(registry_key) {
        return Ok(auth);
    }
    tracing::debug!("No credentials resolved for {registry_key}; using anonymous auth");
    Ok(RegistryAuth::Anonymous)
}

/// Snapshot of the three `OMMX_BASIC_AUTH_*` env vars. Extracted so the
/// classification logic in [`classify_env_credentials`] is unit-testable
/// without touching process env (which is parallel-unsafe under
/// cargo's default test runner).
struct EnvCredentials {
    domain: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

fn auth_from_env(registry_key: &str) -> crate::Result<Option<RegistryAuth>> {
    classify_env_credentials(
        registry_key,
        EnvCredentials {
            domain: env::var("OMMX_BASIC_AUTH_DOMAIN").ok(),
            username: env::var("OMMX_BASIC_AUTH_USERNAME").ok(),
            password: env::var("OMMX_BASIC_AUTH_PASSWORD").ok(),
        },
    )
}

/// `OMMX_BASIC_AUTH_DOMAIN` is the gating flag for the env-var override.
/// If it is set and matches the target registry, both
/// `OMMX_BASIC_AUTH_USERNAME` and `OMMX_BASIC_AUTH_PASSWORD` are
/// mandatory: a CI misconfiguration that sets the domain but forgets a
/// credential variable should error loudly rather than silently fall
/// through to the workstation's `~/.docker/config.json` (which is
/// usually absent in CI) and then to anonymous (which would surface as
/// an unhelpful registry 401).
///
/// When the domain doesn't match the target, the override is
/// intentionally inactive — return `Ok(None)` so docker config is
/// consulted next.
fn classify_env_credentials(
    registry_key: &str,
    creds: EnvCredentials,
) -> crate::Result<Option<RegistryAuth>> {
    let Some(domain) = creds.domain else {
        return Ok(None);
    };
    if domain != registry_key {
        tracing::debug!(
            "OMMX_BASIC_AUTH_DOMAIN={domain} does not match target {registry_key}; \
             falling through to docker config"
        );
        return Ok(None);
    }
    match (creds.username, creds.password) {
        (Some(username), Some(password)) => {
            tracing::info!(
                "Using OMMX_BASIC_AUTH credentials for {registry_key} (user {username})"
            );
            Ok(Some(RegistryAuth::Basic(username, password)))
        }
        (u, p) => {
            let username_state = if u.is_some() { "set" } else { "unset" };
            let password_state = if p.is_some() { "set" } else { "unset" };
            Err(crate::error!(InvalidAuthenticationConfiguration {
                domain,
                username_state,
                password_state,
            }))
        }
    }
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
        Err(_) => {
            // Do not log the upstream error directly. Some variants
            // can carry helper stdout/stderr, which may include
            // credentials or tokens.
            tracing::warn!(
                "Failed to read docker credential for {hostname}; falling through to anonymous"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{remote_error, ImageRef, RemoteArtifactError};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn bounded_map_limits_in_flight_operations() {
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let in_flight = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));

        let output = runtime
            .block_on(bounded_map(0..12, NonZeroUsize::new(3).unwrap(), |item| {
                let in_flight = Arc::clone(&in_flight);
                let maximum = Arc::clone(&maximum);
                async move {
                    let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(current, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    Ok(item)
                }
            }))
            .unwrap();

        assert_eq!(output.len(), 12);
        assert_eq!(maximum.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn bounded_map_returns_a_concurrent_operation_error() {
        let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
        let error = runtime
            .block_on(bounded_map(
                0..4,
                NonZeroUsize::new(2).unwrap(),
                |item| async move {
                    anyhow::ensure!(item != 2, "blob sha256:failed failed");
                    Ok(item)
                },
            ))
            .unwrap_err();

        assert!(error.to_string().contains("sha256:failed"));
    }

    #[test]
    fn artifact_transfer_concurrency_uses_conservative_default() {
        assert_eq!(
            parse_blob_transfer_concurrency(None).unwrap().get(),
            DEFAULT_ARTIFACT_TRANSFER_CONCURRENCY
        );
    }

    #[test]
    fn artifact_transfer_concurrency_accepts_positive_override() {
        assert_eq!(parse_blob_transfer_concurrency(Some("8")).unwrap().get(), 8);
    }

    #[test]
    fn artifact_transfer_concurrency_rejects_invalid_override() {
        for value in ["", "0", "-1", "four"] {
            let error = parse_blob_transfer_concurrency(Some(value)).unwrap_err();
            assert!(error
                .to_string()
                .contains(ARTIFACT_TRANSFER_CONCURRENCY_ENV));
        }
    }

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
    /// crash from a broken `docker-credential-*` binary. The helper's
    /// stdout / stderr is never logged — see the comment in
    /// `classify_docker_credential` about token-leak risk.
    #[test]
    fn classify_helper_failure_falls_through() {
        let auth = classify_docker_credential(
            "ghcr.io",
            Err(CredentialRetrievalError::HelperFailure {
                helper: "docker-credential-broken".to_string(),
                stdout: "{\"Secret\": \"leaked-token\"}".to_string(),
                stderr: "AWS_SECRET_ACCESS_KEY=should-not-appear-in-logs".to_string(),
            }),
        );
        assert!(auth.is_none());
    }

    /// `registry_key` is what we hand to the env-var override comparison
    /// `image_name.registry()` is what we hand to the env-var override
    /// comparison and to `docker_credential::get_credential`. Both
    /// surfaces key on `host[:port]` (matching `docker login
    /// localhost:5000`), not hostname alone — using hostname only
    /// would silently miss credentials on non-443/80 registries.
    #[test]
    fn registry_string_includes_port_when_present() {
        let with_port = ImageRef::parse("localhost:5000/ommx/native-push:tag1").unwrap();
        assert_eq!(with_port.registry(), "localhost:5000");

        let no_port = ImageRef::parse("ghcr.io/jij-inc/ommx:tag1").unwrap();
        assert_eq!(no_port.registry(), "ghcr.io");
    }

    /// `protocol_for` parses the host portion out of the joined
    /// `host[:port]` form inline and switches HTTPS → HTTP for local
    /// addresses. Regressing the inline split (e.g. by treating the
    /// whole `host:port` as the host literal) would lose the
    /// localhost-with-port case.
    #[test]
    fn protocol_for_picks_http_for_localhost_variants() {
        for registry in ["localhost", "localhost:5000", "127.0.0.1", "127.0.0.1:5000"] {
            assert!(
                matches!(protocol_for(registry), ClientProtocol::Http),
                "expected HTTP for local registry {registry}",
            );
        }
        for registry in ["ghcr.io", "registry.example.com:443"] {
            assert!(
                matches!(protocol_for(registry), ClientProtocol::Https),
                "expected HTTPS for remote registry {registry}",
            );
        }
    }

    fn env(domain: Option<&str>, username: Option<&str>, password: Option<&str>) -> EnvCredentials {
        EnvCredentials {
            domain: domain.map(str::to_string),
            username: username.map(str::to_string),
            password: password.map(str::to_string),
        }
    }

    /// Domain unset → override inactive, fall through to docker config.
    #[test]
    fn env_creds_no_domain_falls_through() {
        let out = classify_env_credentials("ghcr.io", env(None, None, None)).unwrap();
        assert!(out.is_none());

        // Username/password set without domain is also fall-through:
        // the user hasn't activated the override.
        let out = classify_env_credentials("ghcr.io", env(None, Some("u"), Some("p"))).unwrap();
        assert!(out.is_none());
    }

    /// Domain set but pointing somewhere else → docker config takes over.
    /// Avoids surprising users who set the var globally on the
    /// workstation but push to a different registry.
    #[test]
    fn env_creds_domain_mismatch_falls_through() {
        let out = classify_env_credentials(
            "ghcr.io",
            env(Some("registry.example.com"), Some("u"), Some("p")),
        )
        .unwrap();
        assert!(out.is_none());
    }

    /// Domain matches + both credentials → Basic auth.
    #[test]
    fn env_creds_full_override_produces_basic() {
        let out = classify_env_credentials(
            "ghcr.io",
            env(Some("ghcr.io"), Some("alice"), Some("secret")),
        )
        .unwrap();
        assert_basic(out, "alice", "secret");
    }

    /// Domain matches but one of the credential vars is missing → loud
    /// error, *not* silent fall-through. This is the CI misconfig case
    /// Codex flagged: setting only USERNAME and forgetting PASSWORD
    /// must not surface as an opaque registry 401 after we fall
    /// through to anonymous.
    #[test]
    fn env_creds_partial_override_errors() {
        let image = ImageRef::parse("ghcr.io/jij-inc/ommx:latest").unwrap();
        for (u, p) in [(Some("alice"), None), (None, Some("secret")), (None, None)] {
            let err = classify_env_credentials("ghcr.io", env(Some("ghcr.io"), u, p))
                .expect_err("partial OMMX_BASIC_AUTH_* should be an error");
            let msg = err.to_string();
            assert!(msg.contains("OMMX_BASIC_AUTH_DOMAIN=ghcr.io"));
            assert!(msg.contains("USERNAME="));
            assert!(msg.contains("PASSWORD="));
            assert!(matches!(
                remote_error::classify_manifest(&image, err),
                RemoteArtifactError::Authentication { .. }
            ));
        }
    }

    #[test]
    fn invalid_remote_response_is_classified_without_message_matching() {
        let image = ImageRef::parse("ghcr.io/jij-inc/ommx:latest").unwrap();
        let source = crate::error!(InvalidRemoteResponse::ContentLengthMismatch {
            digest: "sha256:deadbeef".to_string(),
            content_length: 2,
            expected_size: 1,
        });
        assert!(matches!(
            remote_error::classify_manifest(&image, source),
            RemoteArtifactError::InvalidArtifact { .. }
        ));
    }

    #[test]
    fn blob_stream_io_is_classified_as_transport() {
        let image = ImageRef::parse("ghcr.io/jij-inc/ommx:latest").unwrap();
        let source = blob_stream_error(
            "sha256:deadbeef",
            io::Error::new(io::ErrorKind::ConnectionReset, "connection reset"),
        );
        assert!(matches!(
            remote_error::classify_blob(&image, source),
            RemoteArtifactError::Transport { .. }
        ));
    }
}
