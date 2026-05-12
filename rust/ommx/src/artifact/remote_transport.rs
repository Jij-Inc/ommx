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
//! Both push and pull surfaces are implemented here.
//! `pull_manifest_raw` returns the manifest body verbatim so the
//! digest the registry computes and the digest we store locally agree
//! byte-for-byte; `pull_blob_to_vec` collects a blob into a `Vec<u8>`
//! over `oci-client`'s streaming reader. Layer-blob streaming straight
//! into [`super::local_registry::FileBlobStore`] is a future refinement
//! once `FileBlobStore` grows an `AsyncWrite`-compatible put path.
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
use futures_util::TryStreamExt;
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
    pub(crate) fn new(image_name: &crate::artifact::ImageRef) -> crate::Result<Self> {
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to build tokio runtime for OCI remote transport")?;

        let config = ClientConfig {
            protocol: protocol_for(image_name.hostname()),
            ..ClientConfig::default()
        };
        let client = Client::new(config);
        let auth = resolve_auth(&registry_key(image_name))?;

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
    pub(crate) fn auth_for(
        &self,
        image_name: &crate::artifact::ImageRef,
        operation: RegistryOperation,
    ) -> crate::Result<()> {
        let reference = to_reference(image_name)?;
        self.runtime
            .block_on(self.client.auth(&reference, &self.auth, operation))
            .with_context(|| format!("Failed to authenticate against {reference}"))?;
        Ok(())
    }

    /// Convenience: authenticate for a `Push` request. Most existing
    /// call sites push; the explicit form is [`Self::auth_for`].
    pub(crate) fn auth(&self, image_name: &crate::artifact::ImageRef) -> crate::Result<()> {
        self.auth_for(image_name, RegistryOperation::Push)
    }

    /// Push a single blob to the registry. The caller passes the
    /// pre-computed digest (which the SQLite Local Registry already
    /// stores as the BlobStore key), so the registry-side digest can be
    /// validated without re-hashing. `bytes` is moved into
    /// `oci_client::Client::push_blob` (which takes `Vec<u8>` by
    /// value) so blobs the caller already owns don't get cloned.
    pub(crate) fn push_blob(
        &self,
        image_name: &crate::artifact::ImageRef,
        digest: &str,
        bytes: Vec<u8>,
    ) -> crate::Result<()> {
        let reference = to_reference(image_name)?;
        self.runtime
            .block_on(self.client.push_blob(&reference, bytes, digest))
            .with_context(|| format!("Failed to push blob {digest} to {reference}"))?;
        Ok(())
    }

    /// Push the manifest bytes verbatim with the caller-supplied OCI
    /// `Content-Type`. `oci_client::Client::push_manifest_raw` skips
    /// the typed `OciManifest` round-trip, so the digest stored locally
    /// and the digest the registry computes agree byte-for-byte.
    /// `manifest_bytes` is moved through into `push_manifest_raw` to
    /// avoid cloning a manifest the caller already owns.
    pub(crate) fn push_manifest_bytes(
        &self,
        image_name: &crate::artifact::ImageRef,
        manifest_bytes: Vec<u8>,
        content_type: &str,
    ) -> crate::Result<()> {
        let reference = to_reference(image_name)?;
        let header: HeaderValue = content_type
            .parse()
            .with_context(|| format!("Invalid Content-Type {content_type}"))?;
        self.runtime
            .block_on(
                self.client
                    .push_manifest_raw(&reference, manifest_bytes, header),
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
    pub(crate) fn pull_manifest_raw(
        &self,
        image_name: &crate::artifact::ImageRef,
        accepted_media_types: &[&str],
    ) -> crate::Result<(Vec<u8>, String)> {
        let reference = to_reference(image_name)?;
        let (bytes, digest) = self
            .runtime
            .block_on(
                self.client
                    .pull_manifest_raw(&reference, &self.auth, accepted_media_types),
            )
            .with_context(|| format!("Failed to pull manifest from {reference}"))?;
        Ok((bytes.to_vec(), digest))
    }

    /// Pull a single blob into memory. The caller passes the manifest-
    /// declared `expected_size`; the helper validates registry-reported
    /// `Content-Length` if present, aborts the chunk loop the moment
    /// accumulated bytes exceed `expected_size`, and caps the initial
    /// `Vec::with_capacity` at [`BLOB_PREALLOC_CAP_BYTES`] so a manifest
    /// that lies about a multi-gigabyte size cannot induce an
    /// up-front OOM before any digest check has a chance to fire. The
    /// buffer can still grow past the cap if the actual stream
    /// genuinely produces that many bytes (legitimate large blobs
    /// just pay an extra realloc).
    ///
    /// `oci_client::Client::pull_blob_stream` skips the digest
    /// verification that `pull_blob` does on the streaming reader; the
    /// caller writes the bytes into
    /// [`super::local_registry::FileBlobStore`] which re-derives sha256
    /// during `put_bytes` and then asserts the result matches the
    /// expected digest, so the registry-side digest is enforced exactly
    /// once at the storage boundary rather than twice in network and
    /// store layers.
    pub(crate) fn pull_blob_to_vec(
        &self,
        image_name: &crate::artifact::ImageRef,
        digest: &str,
        expected_size: u64,
    ) -> crate::Result<Vec<u8>> {
        let reference = to_reference(image_name)?;
        let bytes = self
            .runtime
            .block_on(async {
                let resp = self.client.pull_blob_stream(&reference, digest).await?;
                if let Some(content_length) = resp.content_length {
                    anyhow::ensure!(
                        content_length == expected_size,
                        "Registry reported Content-Length {content_length} for blob {digest}, \
                         but the manifest descriptor declares size {expected_size}",
                    );
                }
                // Cap preallocation. A malicious manifest claiming
                // `size = u64::MAX` would otherwise be reduced to
                // `usize::MAX` and OOM on the spot; capping at a
                // sane upper bound forces the registry to actually
                // serve that many bytes (which the accumulated check
                // below catches) before we allocate them.
                let prealloc = expected_size.min(BLOB_PREALLOC_CAP_BYTES) as usize;
                let mut buf = Vec::with_capacity(prealloc);
                let mut accumulated: u64 = 0;
                let mut stream = resp.stream;
                while let Some(chunk) = stream.try_next().await? {
                    accumulated = accumulated
                        .checked_add(chunk.len() as u64)
                        .context("Pulled blob size overflowed u64")?;
                    anyhow::ensure!(
                        accumulated <= expected_size,
                        "Pulled blob bytes for {digest} exceed declared size {expected_size}; \
                         the registry served more data than the manifest descriptor allows",
                    );
                    buf.extend_from_slice(&chunk);
                }
                Ok::<_, anyhow::Error>(buf)
            })
            .with_context(|| format!("Failed to pull blob {digest} from {reference}"))?;
        Ok(bytes)
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

/// Build an `oci_client::Reference` from OMMX's [`ImageRef`]. OMMX
/// owns the parsed form (hostname / port / name / reference fields),
/// but `oci-client` exposes its own `Reference` newtype and parses
/// from a `host[:port]/name:tag` string, so the canonical interchange
/// is the stringified ref rather than a typed conversion.
fn to_reference(image_name: &crate::artifact::ImageRef) -> crate::Result<Reference> {
    let raw = image_name.to_string();
    raw.parse::<Reference>()
        .with_context(|| format!("Invalid OCI image reference: {raw}"))
}

/// `oci-client` defaults to HTTPS; `localhost` registries (used in tests
/// and Docker-in-Docker setups) are HTTP. Apply the same `localhost`
/// heuristic the docker tooling uses so unauthenticated local pushes
/// don't require a custom client config.
fn protocol_for(hostname: &str) -> ClientProtocol {
    if hostname == "localhost" || hostname.starts_with("127.") || hostname.starts_with("::1") {
        ClientProtocol::Http
    } else {
        ClientProtocol::Https
    }
}

/// `host[:port]` registry key for credential lookup. `docker login
/// localhost:5000` and `docker login ghcr.io` write entries under
/// these forms in `~/.docker/config.json`, and `OMMX_BASIC_AUTH_DOMAIN`
/// is documented to match the same shape. Using only `hostname` here
/// would silently miss credentials on any non-443/80 registry.
fn registry_key(image_name: &crate::artifact::ImageRef) -> String {
    match image_name.port() {
        Some(port) => format!("{}:{port}", image_name.hostname()),
        None => image_name.hostname().to_string(),
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
            crate::bail!(
                "OMMX_BASIC_AUTH_DOMAIN={domain} is set (matches target {registry_key}), \
                 but OMMX_BASIC_AUTH_USERNAME={username_state} and \
                 OMMX_BASIC_AUTH_PASSWORD={password_state}. Both variables are required \
                 for the env-var auth override; unset OMMX_BASIC_AUTH_DOMAIN to fall back \
                 to ~/.docker/config.json instead."
            )
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
        Err(e) => {
            // `CredentialRetrievalError::HelperFailure`'s Display
            // includes the helper's stdout and stderr verbatim. A
            // broken helper that succeeded the exec but returned
            // malformed JSON could echo a partial token in stdout, so
            // log only the structural shape of the failure, not the
            // raw bytes. The matching arm names are descriptive
            // enough for diagnosis (the user can re-run the helper
            // directly to see the leaked stderr if they need it).
            let summary = match &e {
                CredentialRetrievalError::HelperCommunicationError => {
                    "HelperCommunicationError".to_string()
                }
                CredentialRetrievalError::MalformedHelperResponse => {
                    "MalformedHelperResponse".to_string()
                }
                CredentialRetrievalError::HelperFailure { helper, .. } => {
                    format!("HelperFailure({helper})")
                }
                CredentialRetrievalError::CredentialDecodingError => {
                    "CredentialDecodingError".to_string()
                }
                CredentialRetrievalError::ConfigReadError => "ConfigReadError".to_string(),
                CredentialRetrievalError::ConfigNotFound
                | CredentialRetrievalError::NoCredentialConfigured => unreachable!(),
            };
            tracing::warn!(
                "Failed to read docker credential for {hostname}: {summary}; \
                 falling through to anonymous"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ImageRef;

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
    /// stdout / stderr is summarised by error variant only, never
    /// logged verbatim — see the comment in `classify_docker_credential`
    /// about token-leak risk from helpers that return malformed JSON.
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
    /// and to `docker_credential::get_credential`. Both surfaces key on
    /// `host[:port]` (matching `docker login localhost:5000`), not
    /// hostname alone — using hostname only would silently miss
    /// credentials on non-443/80 registries.
    #[test]
    fn registry_key_includes_port_when_present() {
        let with_port = ImageRef::parse("localhost:5000/ommx/native-push:tag1").unwrap();
        assert_eq!(registry_key(&with_port), "localhost:5000");

        let no_port = ImageRef::parse("ghcr.io/jij-inc/ommx:tag1").unwrap();
        assert_eq!(registry_key(&no_port), "ghcr.io");
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
        for (u, p) in [(Some("alice"), None), (None, Some("secret")), (None, None)] {
            let err = classify_env_credentials("ghcr.io", env(Some("ghcr.io"), u, p))
                .expect_err("partial OMMX_BASIC_AUTH_* should be an error");
            let msg = err.to_string();
            assert!(msg.contains("OMMX_BASIC_AUTH_DOMAIN=ghcr.io"));
            assert!(msg.contains("USERNAME="));
            assert!(msg.contains("PASSWORD="));
        }
    }
}
