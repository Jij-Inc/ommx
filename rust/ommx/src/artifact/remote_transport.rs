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
//! [`oci-client`]: https://github.com/oras-project/rust-oci-client

use anyhow::Context;
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
    /// Build a transport configured for the given `image_name`'s registry,
    /// reading basic-auth credentials from the `OMMX_BASIC_AUTH_*`
    /// environment variables. When the env vars are absent the transport
    /// runs anonymously, which is sufficient for unauthenticated public
    /// reads but will fail at push time on registries that require auth.
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
        let auth = auth_from_env(&image_name.hostname);

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
    /// `Content-Type`. `oci_client::Client::push_manifest_raw` lets us
    /// publish both OCI Image Manifest and OCI Artifact Manifest bytes
    /// without round-tripping through the typed `OciManifest` enum, so
    /// the digest stored locally and the digest the registry computes
    /// agree byte-for-byte.
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

fn auth_from_env(hostname: &str) -> RegistryAuth {
    let Ok(domain) = env::var("OMMX_BASIC_AUTH_DOMAIN") else {
        tracing::debug!("OMMX_BASIC_AUTH_DOMAIN not set; using anonymous auth");
        return RegistryAuth::Anonymous;
    };
    if domain != hostname {
        tracing::debug!(
            "OMMX_BASIC_AUTH_DOMAIN={domain} does not match target host {hostname}; using anonymous auth"
        );
        return RegistryAuth::Anonymous;
    }
    let (Ok(username), Ok(password)) = (
        env::var("OMMX_BASIC_AUTH_USERNAME"),
        env::var("OMMX_BASIC_AUTH_PASSWORD"),
    ) else {
        tracing::debug!(
            "OMMX_BASIC_AUTH_USERNAME / OMMX_BASIC_AUTH_PASSWORD not both set; using anonymous auth"
        );
        return RegistryAuth::Anonymous;
    };
    tracing::info!("Using OMMX_BASIC_AUTH credentials for {hostname} (user {username})");
    RegistryAuth::Basic(username, password)
}
