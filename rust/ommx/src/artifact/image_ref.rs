//! OMMX-owned image reference type.
//!
//! v3 drops the `ocipkg::ImageName` re-export in favour of [`ImageRef`].
//! The type is a thin newtype around [`oci_spec::distribution::Reference`],
//! which owns the full distribution-reference parser (Docker host
//! heuristic, `name@<digest>` syntax, per-algorithm digest length
//! validation). The newtype exists so the public API surface of `ommx`
//! doesn't carry a foreign-crate type directly and lets us expose the
//! `hostname()` / `port()` / `name()` / `reference()` accessor shape
//! that internal call sites rely on. The legacy v2 disk-cache layout
//! helpers live in
//! [`local_registry::import::legacy`](super::local_registry::import::legacy),
//! not here — v3 storage is SQLite + content-addressed blobs, not a
//! path-tree keyed by image name.

use anyhow::{Context, Result};
use oci_spec::distribution::Reference;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// Tag substituted in when an image reference omits one. Matches the
/// behaviour of `docker pull <name>` and
/// [`oci_spec::distribution::Reference::tag`].
const DEFAULT_TAG: &str = "latest";

/// Parsed OCI image reference owned by the OMMX SDK.
///
/// Backed by [`oci_spec::distribution::Reference`]. Display produces
/// the canonical `host[:port]/name(:tag|@digest)` form and round-trips
/// through [`ImageRef::parse`].
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

    /// Hostname (without port). `oci_spec` keeps registry + port joined
    /// as one `host[:port]` string; split on `:` to surface the host
    /// portion alone.
    pub fn hostname(&self) -> &str {
        match self.0.registry().rsplit_once(':') {
            Some((host, _)) => host,
            None => self.0.registry(),
        }
    }

    /// Optional registry port. Returns `None` for hostnames without
    /// `:port`. Anything after `:` that fails to parse as `u16` is
    /// treated as "no port" (the upstream parser would have rejected
    /// an invalid port already, so this is defensive only).
    pub fn port(&self) -> Option<u16> {
        self.0
            .registry()
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok())
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
    /// tag, otherwise the default `latest` (`oci_spec` only omits
    /// both for the rare bare-name form that hasn't been defaulted
    /// yet — `parse` always populates a tag, so in practice this
    /// branch is unreachable from `parse`-d values).
    pub fn reference(&self) -> &str {
        self.0
            .digest()
            .or_else(|| self.0.tag())
            .unwrap_or(DEFAULT_TAG)
    }

    /// Repository key for the SQLite Local Registry ref store, in the
    /// `host[:port]/repository` shape `docker login` and the OCI
    /// distribution spec both use. Backed by
    /// [`oci_spec::distribution::Reference`]'s `registry()` so the
    /// host:port portion comes out verbatim (no manual port join).
    pub(crate) fn repository_key(&self) -> String {
        format!("{}/{}", self.0.registry(), self.0.repository())
    }
}

impl FromStr for ImageRef {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let reference = input
            .parse::<Reference>()
            .with_context(|| format!("Invalid image reference: {input}"))?;
        Ok(Self(reference))
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
        assert_eq!(r.hostname(), "ghcr.io");
        assert_eq!(r.port(), None);
        assert_eq!(r.name(), "jij-inc/ommx/demo");
        assert_eq!(r.reference(), "v1");
        assert_eq!(r.to_string(), "ghcr.io/jij-inc/ommx/demo:v1");
    }

    #[test]
    fn parses_with_port() {
        let r = ImageRef::parse("localhost:5000/test:tag1").unwrap();
        assert_eq!(r.hostname(), "localhost");
        assert_eq!(r.port(), Some(5000));
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
        assert_eq!(r.hostname(), "docker.io");
        assert_eq!(r.name(), "library/alpine");
        assert_eq!(r.reference(), "latest");
    }

    /// Docker Hub namespaced refs (`namespace/repo:tag`) must default
    /// the hostname to `docker.io` rather than treating `namespace`
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
                r.hostname(),
                "docker.io",
                "{input} should default the hostname",
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
                .hostname(),
            "ghcr.io"
        );
        let with_port = ImageRef::parse("localhost:5000/repo:tag").unwrap();
        assert_eq!(with_port.hostname(), "localhost");
        assert_eq!(with_port.port(), Some(5000));
        assert_eq!(
            ImageRef::parse("localhost/repo:tag").unwrap().hostname(),
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
}
