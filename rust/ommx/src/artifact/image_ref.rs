//! OMMX-owned image reference type.
//!
//! v3 drops the `ocipkg::ImageName` re-export in favour of [`ImageRef`].
//! The type is a thin newtype around [`oci_spec::distribution::Reference`],
//! which owns the full distribution-reference parser (Docker host
//! heuristic, `name@<digest>` syntax, per-algorithm digest length
//! validation). The newtype exists so the public API surface of `ommx`
//! doesn't carry a foreign-crate type directly. The accessors
//! ([`Self::registry`], [`Self::name`], [`Self::reference`]) follow
//! the OCI distribution-spec shape: `registry()` returns the joined
//! `host[:port]` form verbatim, mirroring
//! [`oci_spec::distribution::Reference::registry`]. The legacy v2
//! disk-cache layout helpers live in
//! [`local_registry::import::legacy`](super::local_registry::import::legacy),
//! not here — v3 storage is SQLite + content-addressed blobs, not a
//! path-tree keyed by image name.

use anyhow::{Context, Result};
use oci_spec::distribution::Reference;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt, str::FromStr};

/// Tag substituted in when an image reference omits one. Matches the
/// behaviour of `docker pull <name>` and
/// [`oci_spec::distribution::Reference::tag`].
const DEFAULT_TAG: &str = "latest";

/// Parsed OCI image reference owned by the OMMX SDK.
///
/// Backed by [`oci_spec::distribution::Reference`]. Display produces
/// the canonical `host[:port]/name(:tag|@digest)` form and round-trips
/// through [`ImageRef::parse`].
///
/// # Canonicalisation invariant
///
/// **Every `ImageRef` value is in canonical form**:
/// 1. [`canonicalize_legacy_docker_hub_host`] has rewritten any
///    `registry-1.docker.io/` prefix (ocipkg's v2 default) to
///    `docker.io/`.
/// 2. `oci_spec::distribution::Reference::split_domain` has
///    normalised the host — `index.docker.io` aliases collapse to
///    `docker.io`, and single-segment Docker Hub names gain the
///    implicit `library/` repository prefix.
///
/// The SQLite Local Registry relies on this invariant: the
/// `(name, reference)` columns are populated from
/// [`Self::repository_key`] and [`Self::reference`], both of which
/// read out of the inner canonical [`Reference`]. A non-canonical
/// `ImageRef` reaching the index would silently route the same image
/// to duplicate SQLite rows, breaking lookups across spellings
/// (`alpine` vs `docker.io/library/alpine:latest` vs
/// `registry-1.docker.io/alpine:latest`).
///
/// The invariant is upheld structurally rather than by run-time
/// assertion: the inner [`Reference`] is private, and the only
/// construction paths are
/// - [`Self::parse`] / `<Self as FromStr>::from_str` — applies both
///   canonicalisation layers.
/// - `<Self as Deserialize>::deserialize` — routes through
///   [`Self::parse`].
/// - [`Self::from_repository_and_reference`] — also routes through
///   [`Self::parse`].
///
/// **Adding any new constructor that bypasses [`Self::parse`] would
/// break the invariant.** A `pub fn from_inner(Reference) -> Self`,
/// a `From<Reference> for ImageRef` impl, or exposing the inner
/// field would let unnormalised data leak into the SQLite key path.
/// Future constructors must either route through [`Self::parse`] or
/// apply both canonicalisation layers explicitly before constructing
/// `Self(...)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageRef(Reference);

impl ImageRef {
    /// Parse a string image reference. Delegates to
    /// [`oci_spec::distribution::Reference`], which accepts
    /// `host[:port]/name:tag`, `host[:port]/name@<digest>`, the
    /// combined `tag@<digest>` form, and Docker-Hub shorthand like
    /// `library/ubuntu` (defaulted under `docker.io`).
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_str(input)
    }

    /// Registry hostname plus optional port (`host[:port]`), as a
    /// single string in the OCI distribution-spec canonical shape.
    /// Equivalent to
    /// [`oci_spec::distribution::Reference::registry`]; the joined
    /// form is exactly what `docker login` writes into
    /// `~/.docker/config.json` and what the SQLite Local Registry's
    /// `name` column carries. Callers that need the host portion
    /// alone (e.g. a `localhost` heuristic) parse it inline; OMMX
    /// does not expose `hostname()` / `port()` split accessors
    /// because they were an ocipkg-shape artefact and every internal
    /// consumer ended up rejoining them back to `host[:port]` at the
    /// call site.
    pub fn registry(&self) -> &str {
        self.0.registry()
    }

    /// Repository path (the part between the registry and the
    /// tag/digest). Equivalent to
    /// [`oci_spec::distribution::Reference::repository`].
    pub fn name(&self) -> &str {
        self.0.repository()
    }

    /// Single string view of the tag-or-digest portion. Prefers the
    /// digest when both are present (digest pins the image immutably
    /// and is what the SQLite Local Registry stores), otherwise the
    /// tag, otherwise the default `latest`. The combined
    /// `name:tag@<digest>` form silently drops the tag here — OMMX
    /// has no code path that produces the combined form, so this is
    /// theoretical for SDK-internal use, but external callers who
    /// pass it should be aware that round-tripping through
    /// `(repository_key, reference)` keeps only the digest.
    /// `oci_spec::distribution::Reference::from_str` always sets at
    /// least one of `tag` or `digest` (a parse with neither falls
    /// back to `latest` inside oci-spec itself), so the
    /// `DEFAULT_TAG` fallback here is defensive only — it would only
    /// fire for an [`ImageRef`] constructed without going through
    /// `parse`, which is currently impossible from public surface.
    pub fn reference(&self) -> &str {
        self.0
            .digest()
            .or_else(|| self.0.tag())
            .unwrap_or(DEFAULT_TAG)
    }

    /// Repository key for the SQLite Local Registry ref store, in the
    /// `host[:port]/repository` shape `docker login` and the OCI
    /// distribution spec both use.
    pub(crate) fn repository_key(&self) -> String {
        format!("{}/{}", self.registry(), self.name())
    }

    /// Borrow the inner [`oci_spec::distribution::Reference`]. This is
    /// the same type [`oci_client`] uses for its remote-transport API
    /// (`oci_client::Reference` is a `pub use` of
    /// `oci_spec::distribution::Reference`), so callers can hand the
    /// borrowed reference to [`oci_client::Client`] methods without
    /// re-parsing the [`Display`] form. Crate-private to keep the
    /// public surface decoupled from `oci_spec` — see the type-level
    /// canonicalisation invariant.
    pub(crate) fn as_inner(&self) -> &Reference {
        &self.0
    }

    /// Build an [`ImageRef`] from the SQLite Local Registry's stored
    /// `(name, reference)` pair (or the v2 disk-cache path components).
    /// Picks the OCI canonical separator at reassembly: `:` for tags,
    /// `@` for digests. Using `:` unconditionally — as earlier code did
    /// — makes
    /// [`oci_spec::distribution::Reference`] reject every digest-pinned
    /// ref, since `name:algorithm:hex` is not in its accepted grammar.
    pub(crate) fn from_repository_and_reference(name: &str, reference: &str) -> Result<Self> {
        let separator = if reference.contains(':') { '@' } else { ':' };
        Self::parse(&format!("{name}{separator}{reference}"))
    }
}

impl FromStr for ImageRef {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let canonical = canonicalize_legacy_docker_hub_host(input);
        let reference = canonical
            .parse::<Reference>()
            .with_context(|| format!("Invalid image reference: {input}"))?;
        Ok(Self(reference))
    }
}

/// Rewrite ocipkg's legacy Docker Hub default hostname
/// (`registry-1.docker.io`) to the OCI distribution-spec canonical
/// (`docker.io`) before delegating to
/// [`oci_spec::distribution::Reference`].
///
/// **Why**: SDK v2 used ocipkg, which defaulted bare image names
/// (`alpine`, `ubuntu:20.04`) to the hostname `registry-1.docker.io`
/// — Docker's actual API endpoint, but not a canonical OCI
/// distribution hostname. v2 caches on disk and v2 archive
/// annotations carry that hostname verbatim. v3 uses
/// `oci_spec::distribution::Reference`, whose `split_domain`
/// normalises `docker.io` (and only `docker.io`, plus its legacy
/// alias `index.docker.io`) by adding the `library/` prefix for
/// single-segment names. Without this shim, the same image surfaces
/// under two distinct SQLite keys depending on which side of the
/// v2 → v3 boundary the string was produced:
///
/// - `Artifact.load("alpine")` → `docker.io/library/alpine`
/// - `import_legacy_local_registry` reading a v2 annotation
///   `registry-1.docker.io/alpine:latest` →
///   `registry-1.docker.io/alpine`
///
/// Rewriting the prefix here collapses both paths onto the same
/// canonical key. The rewrite is bounded to the exact prefix
/// `"registry-1.docker.io/"` (with trailing slash) so adjacent
/// hostnames like `registry-1.docker.io.example/foo` are left
/// alone.
fn canonicalize_legacy_docker_hub_host(input: &str) -> Cow<'_, str> {
    if let Some(rest) = input.strip_prefix("registry-1.docker.io/") {
        Cow::Owned(format!("docker.io/{rest}"))
    } else {
        Cow::Borrowed(input)
    }
}

impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `oci_spec::distribution::Reference` already canonicalises
        // tag refs to `name:tag` and digest refs to `name@digest`;
        // its Display impl is what we want.
        fmt::Display::fmt(&self.0, f)
    }
}

impl Serialize for ImageRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialise as the canonical string rather than the struct
        // form `oci_spec::distribution::Reference` defaults to, so
        // OMMX's on-the-wire representation is the same string the
        // user would type at the CLI.
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ImageRef {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_form() {
        let r = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v1").unwrap();
        assert_eq!(r.registry(), "ghcr.io");
        assert_eq!(r.name(), "jij-inc/ommx/demo");
        assert_eq!(r.reference(), "v1");
        assert_eq!(r.to_string(), "ghcr.io/jij-inc/ommx/demo:v1");
    }

    #[test]
    fn parses_with_port() {
        let r = ImageRef::parse("localhost:5000/test:tag1").unwrap();
        assert_eq!(r.registry(), "localhost:5000");
        assert_eq!(r.name(), "test");
        assert_eq!(r.reference(), "tag1");
        assert_eq!(r.to_string(), "localhost:5000/test:tag1");
    }

    /// `oci_spec::distribution::Reference` defaults Docker-Hub
    /// shorthand to `docker.io/library/<name>:latest`, matching what
    /// `docker pull <name>` resolves to. Verify the SDK exposes that
    /// behaviour through the accessors.
    #[test]
    fn defaults_bare_name_to_docker_hub_with_library_prefix() {
        let r = ImageRef::parse("alpine").unwrap();
        assert_eq!(r.registry(), "docker.io");
        assert_eq!(r.name(), "library/alpine");
        assert_eq!(r.reference(), "latest");
    }

    /// Docker Hub namespaced refs (`namespace/repo:tag`) must default
    /// the registry to `docker.io` rather than treating `namespace`
    /// as a registry. `docker pull library/ubuntu:20.04` resolves to
    /// `docker.io/library/ubuntu:20.04`, so OMMX should parse the
    /// same way — without this heuristic the SDK would silently
    /// route the ref to a non-existent `https://library/` registry.
    #[test]
    fn docker_namespaced_refs_default_to_docker_hub() {
        for (input, name) in [
            ("library/ubuntu:20.04", "library/ubuntu"),
            ("jij-inc/ommx:latest", "jij-inc/ommx"),
        ] {
            let r = ImageRef::parse(input).unwrap();
            assert_eq!(
                r.registry(),
                "docker.io",
                "{input} should default the registry",
            );
            assert_eq!(r.name(), name, "{input} should keep the namespace");
        }
    }

    /// The host heuristic activates on `.` (domain), `:` (port), or the
    /// literal `localhost`. A single bare segment without any of those
    /// is not a hostname — see `defaults_bare_name_to_docker_hub_with_library_prefix`
    /// for the alternative branch.
    #[test]
    fn host_heuristic_activates_on_dot_port_or_localhost() {
        assert_eq!(
            ImageRef::parse("ghcr.io/jij-inc/ommx:v1")
                .unwrap()
                .registry(),
            "ghcr.io"
        );
        let with_port = ImageRef::parse("localhost:5000/repo:tag").unwrap();
        assert_eq!(with_port.registry(), "localhost:5000");
        assert_eq!(
            ImageRef::parse("localhost/repo:tag").unwrap().registry(),
            "localhost",
        );
    }

    /// OCI standard digest spelling `name@algorithm:hex` parses and
    /// round-trips through Display.
    #[test]
    fn accepts_at_digest_and_round_trips() {
        let s = "ghcr.io/jij-inc/ommx@sha256:0011223344556677889900112233445566778899001122334455667788990011";
        let r = ImageRef::parse(s).unwrap();
        assert_eq!(r.name(), "jij-inc/ommx");
        assert_eq!(
            r.reference(),
            "sha256:0011223344556677889900112233445566778899001122334455667788990011"
        );
        assert_eq!(r.to_string(), s);
    }

    /// Tag references keep the `:` separator on Display (the upstream
    /// `oci_spec::distribution::Reference` Display impl handles this).
    #[test]
    fn tag_references_keep_colon_separator_on_display() {
        let r = ImageRef::parse("ghcr.io/jij-inc/ommx:v1").unwrap();
        assert_eq!(r.to_string(), "ghcr.io/jij-inc/ommx:v1");
    }

    /// `oci_spec::distribution::Reference` validates per-algorithm
    /// digest length (sha256 = 64 hex, sha384 = 96, sha512 = 128).
    /// A short-hex digest should fail at parse time rather than reach
    /// the registry.
    #[test]
    fn rejects_short_digest() {
        assert!(ImageRef::parse("ghcr.io/foo@sha256:abc").is_err());
    }

    /// IPv6 host syntax — bracketed or bare — sits outside
    /// `oci_spec::distribution::Reference`'s grammar (ASCII alnum +
    /// `.` + optional `:port`). The transport layer's
    /// `protocol_for` heuristic splits `host[:port]` on `:` to
    /// extract the host for a localhost check; pin the upstream
    /// rejection so that split stays safe.
    #[test]
    fn rejects_ipv6_host_syntax() {
        for input in [
            "[::1]/repo:tag",
            "[::1]:5000/repo:tag",
            "[2001:db8::1]/repo:tag",
            "::1/repo:tag",
        ] {
            assert!(
                ImageRef::parse(input).is_err(),
                "expected oci_spec to reject IPv6 host {input}; revisit protocol_for if it starts accepting them",
            );
        }
    }

    #[test]
    fn rejects_invalid_capital_in_name() {
        assert!(ImageRef::parse("ghcr.io/Foo:v1").is_err());
    }

    /// `@` is only valid as the digest separator. A non-digest after
    /// `@` fails the upstream parser.
    #[test]
    fn rejects_at_sign_in_non_digest_reference() {
        assert!(ImageRef::parse("ghcr.io/foo@nottag").is_err());
    }

    #[test]
    fn serde_round_trips_through_string() {
        let r = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v1").unwrap();
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"ghcr.io/jij-inc/ommx/demo:v1\"");
        let r2: ImageRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    /// `repository_key` is the SQLite Local Registry's `name` column
    /// value — `host[:port]/repository`. With ports it must include
    /// the port; without ports it's just `host/repository`.
    #[test]
    fn repository_key_format() {
        let with_port = ImageRef::parse("localhost:5000/ommx/test:tag1").unwrap();
        assert_eq!(with_port.repository_key(), "localhost:5000/ommx/test");
        let no_port = ImageRef::parse("ghcr.io/jij-inc/ommx:tag1").unwrap();
        assert_eq!(no_port.repository_key(), "ghcr.io/jij-inc/ommx");
    }

    /// `from_repository_and_reference` is what `get_images()` (and the
    /// legacy v2 path decoder) use to reassemble an [`ImageRef`] from a
    /// `(repository, reference)` pair stored in the SQLite Local
    /// Registry. The earlier code joined unconditionally with `:`,
    /// which `oci_spec` rejects for digest references (`name:sha256:...`
    /// is not in its accepted grammar). Verify the helper picks `@`
    /// for digests and `:` for tags so `get_images()` survives a
    /// digest-pinned ref in the registry.
    #[test]
    fn from_repository_and_reference_picks_at_for_digests() {
        let tag = ImageRef::from_repository_and_reference("ghcr.io/jij-inc/ommx", "v1").unwrap();
        assert_eq!(tag.to_string(), "ghcr.io/jij-inc/ommx:v1");

        let digest = "sha256:0011223344556677889900112233445566778899001122334455667788990011";
        let digest_ref =
            ImageRef::from_repository_and_reference("ghcr.io/jij-inc/ommx", digest).unwrap();
        assert_eq!(
            digest_ref.to_string(),
            format!("ghcr.io/jij-inc/ommx@{digest}"),
            "digest reference must use `@`, not `:`, to survive oci_spec parsing",
        );
    }

    /// **v2 compat invariant**: the ocipkg legacy default hostname
    /// `registry-1.docker.io` (used for bare image names like `alpine`)
    /// must parse to the same canonical [`ImageRef`] as the oci-spec
    /// normalised forms `alpine` / `docker.io/alpine`. Without this,
    /// a v2 cache imported from a manifest annotation under
    /// `registry-1.docker.io/alpine:latest` would land under a
    /// different SQLite key than `Artifact.load("alpine")` later
    /// queries, and the cache would be silently invisible.
    #[test]
    fn parse_collapses_ocipkg_docker_hub_host_to_canonical() {
        let legacy = ImageRef::parse("registry-1.docker.io/alpine:latest").unwrap();
        let bare = ImageRef::parse("alpine").unwrap();
        let canonical = ImageRef::parse("docker.io/alpine:latest").unwrap();
        assert_eq!(
            legacy, bare,
            "registry-1.docker.io/alpine:latest must parse to the same ImageRef as the bare name",
        );
        assert_eq!(
            legacy, canonical,
            "registry-1.docker.io/alpine:latest must parse to the same ImageRef as docker.io/alpine:latest",
        );
        // The collapsed key is what the SQLite Local Registry stores
        // under, so all three forms must produce the same repository
        // key when looked up.
        assert_eq!(legacy.repository_key(), "docker.io/library/alpine");
        assert_eq!(legacy.to_string(), "docker.io/library/alpine:latest");
    }

    /// Multi-segment Docker Hub names (`registry-1.docker.io/<org>/<repo>`)
    /// must collapse to the same key as the canonical `docker.io/<org>/<repo>`
    /// form. oci-spec skips the `library/` prefix when the repository
    /// already has a slash, so the result is `docker.io/<org>/<repo>`
    /// from both sides.
    #[test]
    fn parse_collapses_multi_segment_docker_hub_host() {
        let legacy = ImageRef::parse("registry-1.docker.io/jij-inc/ommx:v1").unwrap();
        let canonical = ImageRef::parse("docker.io/jij-inc/ommx:v1").unwrap();
        assert_eq!(legacy, canonical);
        assert_eq!(legacy.repository_key(), "docker.io/jij-inc/ommx");
        assert_eq!(legacy.to_string(), "docker.io/jij-inc/ommx:v1");
    }

    /// Digest-pinned legacy refs must collapse the host the same way,
    /// so v2 archive annotations like
    /// `registry-1.docker.io/alpine@sha256:...` round-trip to the
    /// canonical SQLite key.
    #[test]
    fn parse_collapses_legacy_docker_hub_host_with_digest() {
        let digest = "sha256:0011223344556677889900112233445566778899001122334455667788990011";
        let legacy = ImageRef::parse(&format!("registry-1.docker.io/alpine@{digest}")).unwrap();
        let canonical = ImageRef::parse(&format!("docker.io/alpine@{digest}")).unwrap();
        assert_eq!(legacy, canonical);
        assert_eq!(legacy.repository_key(), "docker.io/library/alpine");
    }

    /// The shim must only fire on the exact prefix
    /// `"registry-1.docker.io/"` (with trailing slash). A registry
    /// hostname that *contains* `registry-1.docker.io` as a substring
    /// is a different domain and must be left alone, otherwise we
    /// silently misroute pulls.
    #[test]
    fn parse_does_not_rewrite_lookalike_hosts() {
        let lookalike = ImageRef::parse("registry-1.docker.io.example/foo:v1").unwrap();
        assert_eq!(
            lookalike.registry(),
            "registry-1.docker.io.example",
            "substring-matching hostnames must not be rewritten to docker.io",
        );
        assert_eq!(lookalike.name(), "foo");
    }

    /// A bare `registry-1.docker.io` string (no `/`) does not match
    /// the trailing-slash prefix and must not be rewritten — without
    /// the slash there's no `<rest>` to splice on. oci-spec then
    /// treats the whole string as a single-segment repository name
    /// under the default Docker Hub registry (so the parsed
    /// repository ends up as `library/registry-1.docker.io`). That's
    /// a quirky-but-consistent oci-spec outcome; the assertion here
    /// pins **the shim did not run** — if it had, the input would
    /// have become `docker.io/<empty>`, which oci-spec rejects.
    #[test]
    fn parse_does_not_rewrite_bare_registry_host_string() {
        let parsed = ImageRef::parse("registry-1.docker.io").unwrap();
        assert_eq!(parsed.name(), "library/registry-1.docker.io");
        assert_eq!(parsed.registry(), "docker.io");
    }

    /// Structural canonicalisation invariant (see the `ImageRef`
    /// type-level rustdoc). Every spelling of the same Docker Hub
    /// image must yield `==`-equal `ImageRef` values, because both
    /// the v2 shim and `oci_spec::distribution::Reference::split_domain`
    /// run inside `parse` and converge on the canonical form
    /// `docker.io/library/alpine:latest`. A regression that lets an
    /// uncanonicalised `Reference` through (e.g. a future
    /// `From<Reference>` impl) would surface here as one of these
    /// spellings parsing to a different `ImageRef` than the others.
    #[test]
    fn canonicalisation_invariant_collapses_every_docker_hub_spelling() {
        let spellings = [
            "alpine",
            "alpine:latest",
            "docker.io/alpine",
            "docker.io/alpine:latest",
            "docker.io/library/alpine:latest",
            "index.docker.io/alpine:latest",
            "index.docker.io/library/alpine:latest",
            "registry-1.docker.io/alpine:latest",
            "registry-1.docker.io/library/alpine:latest",
        ];
        let canonical = ImageRef::parse(spellings[0]).unwrap();
        for spelling in &spellings[1..] {
            let parsed = ImageRef::parse(spelling)
                .unwrap_or_else(|e| panic!("parse({spelling}) failed: {e}"));
            assert_eq!(
                parsed,
                canonical,
                "spelling {spelling} broke the canonicalisation invariant: \
                 produced {parsed} (repository_key={}) but canonical form is {canonical} \
                 (repository_key={})",
                parsed.repository_key(),
                canonical.repository_key(),
            );
        }
        // Spot-check the final canonical shape so a regression that
        // breaks *both* sides of the assertion (e.g. removing the
        // shim AND the oci-spec dep) still gets caught.
        assert_eq!(canonical.repository_key(), "docker.io/library/alpine");
        assert_eq!(canonical.reference(), "latest");
        assert_eq!(canonical.to_string(), "docker.io/library/alpine:latest");
    }
}
