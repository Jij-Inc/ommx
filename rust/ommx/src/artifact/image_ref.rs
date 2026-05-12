//! OMMX-owned image reference type.
//!
//! v3 drops the `ocipkg::ImageName` re-export in favour of
//! [`ImageRef`]. The two types parse the same `host[:port]/name:tag`
//! shape and expose the same `hostname` / `port` / `name` / `reference`
//! accessors plus the `as_path` / `from_path` legacy-layout helpers
//! used by the v2 → v3 disk-cache import path. Display round-trips
//! through `parse` byte-for-byte.
//!
//! The struct exists so the public API surface of `ommx` doesn't carry
//! a type from an external crate — `ocipkg` is unmaintained, and any
//! ergonomic change to the v3 SDK should not have to wait on it.

use anyhow::{bail, Context, Result};
use oci_spec::image::Digest;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
};

/// Hostname used when an image reference omits the registry, matching
/// `docker pull <name>` behaviour.
const DEFAULT_HOSTNAME: &str = "registry-1.docker.io";

/// Tag substituted in when an image reference omits one.
const DEFAULT_TAG: &str = "latest";

/// Repository-name regex from the OCI distribution spec v1.1.0:
/// `[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*(\/[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*)*`.
/// Matched at construction so `ImageRef::parse` rejects names that the
/// registry would later reject with a 4xx.
static NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*(/[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*)*$")
        .expect("static OCI name regex compiles")
});

/// Tag-shape regex from the OCI distribution spec: 1-128 chars,
/// `[a-zA-Z0-9_]` head followed by `[a-zA-Z0-9._-]`. Digest references
/// fail this match and fall through to `Digest::from_str`.
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}$").expect("static OCI tag regex compiles")
});

/// Parsed OCI image reference owned by the OMMX SDK.
///
/// Fields are private; use the accessors. The string shape produced
/// by [`fmt::Display`] is `host[:port]/name:reference` and round-trips
/// through [`ImageRef::parse`] / [`FromStr`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageRef {
    hostname: String,
    port: Option<u16>,
    name: String,
    reference: String,
}

impl ImageRef {
    /// Parse a string image reference. Accepts `host[:port]/name:tag`,
    /// `name:tag`, `host[:port]/name`, and `name` — the last two
    /// default the missing component (`registry-1.docker.io` /
    /// `latest`).
    pub fn parse(input: &str) -> Result<Self> {
        Self::from_str(input)
    }

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn port(&self) -> Option<u16> {
        self.port
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// Encode the ref as `{hostname}/{name}/__{reference}` (or
    /// `{hostname}__{port}/{name}/__{reference}` when a port is set).
    /// This is the layout the SDK v2 disk-cache local registry used,
    /// and the v3 SQLite import path still walks it via
    /// [`Self::from_path`] when migrating user data.
    ///
    /// The legacy encoding maps `:` to `__`, inherited byte-for-byte
    /// from SDK v2. That makes tags containing `__` (which the OCI
    /// distribution tag grammar otherwise allows) ambiguous with
    /// digest separators on round-trip — a path written from tag
    /// `my__tag` decodes back as ref `my:tag` and fails validation.
    /// OMMX-generated refs never use `__` in tags, so the v2 → v3
    /// import path is unaffected; the round-trip property only holds
    /// for refs whose tag does not contain `__`. This intentionally
    /// preserves on-disk compatibility with v2 caches rather than
    /// switching to a percent-encoded layout that would invalidate
    /// existing user data.
    pub fn as_path(&self) -> PathBuf {
        let reference = self.reference.replace(':', "__");
        let host = match self.port {
            Some(port) => format!("{}__{port}", self.hostname),
            None => self.hostname.clone(),
        };
        PathBuf::from(format!("{host}/{}/__{reference}", self.name))
    }

    /// Inverse of [`Self::as_path`]. Returns an error when the path
    /// shape doesn't match the encoding (so a stray directory inside
    /// the legacy local registry root surfaces a clear error during
    /// import rather than producing a corrupted ref).
    pub fn from_path(path: &Path) -> Result<Self> {
        let components = path
            .components()
            .map(|c| {
                c.as_os_str()
                    .to_str()
                    .context("Path includes a non UTF-8 component")
            })
            .collect::<Result<Vec<&str>>>()?;
        if components.len() < 3 {
            bail!(
                "Path for image ref must contain registry, name, and tag components: {}",
                path.display()
            );
        }

        let registry = components[0];
        let (hostname, port) = if let Some((host, port)) = registry.split_once("__") {
            let port = port.parse::<u16>().context("Invalid port number")?;
            (host.to_string(), Some(port))
        } else {
            (registry.to_string(), None)
        };

        let n = components.len();
        let name = components[1..n - 1].join("/");
        validate_name(&name)?;

        let reference = components[n - 1]
            .strip_prefix("__")
            .with_context(|| format!("Missing tag prefix in path: {}", path.display()))?
            .replace("__", ":");
        validate_reference(&reference)?;

        Ok(Self {
            hostname,
            port,
            name,
            reference,
        })
    }
}

impl FromStr for ImageRef {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let (hostname, port, rest) = split_registry(input)?;
        let (name, reference) = parse_name_reference(rest)?;
        Ok(Self {
            hostname,
            port,
            name,
            reference,
        })
    }
}

impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Canonicalise on Display: digest references use the OCI
        // standard `name@<digest>` form, tag references use
        // `name:<tag>`. Both forms are accepted on parse.
        let separator = if is_digest_reference(&self.reference) {
            '@'
        } else {
            ':'
        };
        match self.port {
            Some(port) => write!(
                f,
                "{}:{port}/{}{separator}{}",
                self.hostname, self.name, self.reference
            ),
            None => write!(
                f,
                "{}/{}{separator}{}",
                self.hostname, self.name, self.reference
            ),
        }
    }
}

/// Apply the Docker / OCI distribution convention to decide whether the
/// first `/`-separated segment is a registry host or part of the
/// repository name. A leading segment is treated as a host only when it
/// equals `localhost`, contains a `.` (a hostname always does), or
/// contains a `:` (an explicit port). Otherwise the full input is a
/// repository path under the default registry — so `library/ubuntu`
/// parses as `registry-1.docker.io/library/ubuntu`, matching what
/// `docker pull` would resolve.
fn split_registry(input: &str) -> Result<(String, Option<u16>, &str)> {
    if let Some((first, rest)) = input.split_once('/') {
        if first == "localhost" || first.contains('.') || first.contains(':') {
            let (hostname, port) = match first.split_once(':') {
                Some((host, port)) => {
                    let port = port.parse::<u16>().context("Invalid port number")?;
                    (host.to_string(), Some(port))
                }
                None => (first.to_string(), None),
            };
            return Ok((hostname, port, rest));
        }
    }
    Ok((DEFAULT_HOSTNAME.to_string(), None, input))
}

/// Split the post-registry portion into `(name, reference)`. Accepted
/// shapes, in priority order:
///
/// 1. `name@<digest>` — OCI standard digest form. `@` cannot appear in
///    valid tags, so splitting on `@` is unambiguous.
/// 2. `name:<reference>` — tag form, or the legacy `name:algorithm:hex`
///    digest spelling that the OCI distribution spec still accepts.
///    `split_once(':')` finds the first `:`, leaving any trailing
///    `algorithm:hex` intact for [`validate_reference`] to recognise.
/// 3. `name` — bare path; tag defaults to `latest`.
fn parse_name_reference(rest: &str) -> Result<(String, String)> {
    if let Some((name, digest)) = rest.split_once('@') {
        validate_name(name)?;
        Digest::from_str(digest).with_context(|| format!("Invalid digest reference: {digest}"))?;
        return Ok((name.to_string(), digest.to_string()));
    }
    if let Some((name, reference)) = rest.split_once(':') {
        validate_name(name)?;
        validate_reference(reference)?;
        return Ok((name.to_string(), reference.to_string()));
    }
    validate_name(rest)?;
    Ok((rest.to_string(), DEFAULT_TAG.to_string()))
}

/// True iff `reference` parses as an OCI digest (`algorithm:hex`).
/// Used by [`Display`] to pick `@` vs `:` separator.
fn is_digest_reference(reference: &str) -> bool {
    Digest::from_str(reference).is_ok()
}

impl Serialize for ImageRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
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

fn validate_name(name: &str) -> Result<()> {
    if NAME_RE.is_match(name) {
        Ok(())
    } else {
        bail!("Invalid image name: {name}")
    }
}

/// Tag references match `TAG_RE`; digest references (`sha256:...`) match
/// `Digest::from_str`. Both forms are valid `<reference>` per the OCI
/// distribution spec.
fn validate_reference(reference: &str) -> Result<()> {
    if TAG_RE.is_match(reference) {
        return Ok(());
    }
    if reference.contains(':') {
        Digest::from_str(reference)
            .with_context(|| format!("Invalid digest reference: {reference}"))?;
        return Ok(());
    }
    bail!("Invalid image reference: {reference}")
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

    #[test]
    fn defaults_hostname_when_omitted() {
        let r = ImageRef::parse("ubuntu:20.04").unwrap();
        assert_eq!(r.hostname(), "registry-1.docker.io");
        assert_eq!(r.name(), "ubuntu");
        assert_eq!(r.reference(), "20.04");
    }

    #[test]
    fn defaults_reference_when_omitted() {
        let r = ImageRef::parse("alpine").unwrap();
        assert_eq!(r.reference(), "latest");
    }

    /// Docker Hub namespaced refs (`namespace/repo:tag`) must default
    /// the hostname rather than treating `namespace` as a registry.
    /// `docker pull library/ubuntu:20.04` resolves to
    /// `registry-1.docker.io/library/ubuntu:20.04`, so OMMX should
    /// parse the same way; without this heuristic the SDK would
    /// silently route `library/ubuntu:20.04` to an `https://library/`
    /// registry that does not exist.
    #[test]
    fn docker_namespaced_refs_default_to_docker_hub() {
        for input in ["library/ubuntu:20.04", "jij-inc/ommx:latest"] {
            let r = ImageRef::parse(input).unwrap();
            assert_eq!(
                r.hostname(),
                "registry-1.docker.io",
                "{input} should default the hostname",
            );
            assert!(r.name().contains('/'), "{input} should keep the namespace");
        }
    }

    /// The host heuristic activates on `.` (domain), `:` (port), or the
    /// literal `localhost`. A single bare segment without any of those
    /// is not a hostname.
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

    /// Legacy digest spelling `name:algorithm:hex` is accepted (the OCI
    /// distribution spec still recognises it), but the canonical
    /// Display form uses the standard `name@algorithm:hex` separator.
    #[test]
    fn accepts_legacy_colon_digest_and_canonicalises_to_at() {
        let s = "quay.io/jitesoft/alpine:sha256:6755355f801f8e3694bffb1a925786813462cea16f1ce2b0290b6a48acf2500c";
        let r = ImageRef::parse(s).unwrap();
        assert_eq!(r.name(), "jitesoft/alpine");
        assert_eq!(
            r.reference(),
            "sha256:6755355f801f8e3694bffb1a925786813462cea16f1ce2b0290b6a48acf2500c"
        );
        assert_eq!(
            r.to_string(),
            "quay.io/jitesoft/alpine@sha256:6755355f801f8e3694bffb1a925786813462cea16f1ce2b0290b6a48acf2500c",
            "Display must canonicalise digest refs to the `@` separator",
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

    /// Tag references keep the `:` separator on Display.
    #[test]
    fn tag_references_keep_colon_separator_on_display() {
        let r = ImageRef::parse("ghcr.io/jij-inc/ommx:v1").unwrap();
        assert_eq!(r.to_string(), "ghcr.io/jij-inc/ommx:v1");
    }

    #[test]
    fn rejects_invalid_capital_in_name() {
        assert!(ImageRef::parse("ghcr.io/Foo:v1").is_err());
    }

    /// `@` is only valid as the digest separator. A tag string that
    /// contains `@` is rejected on the digest side because the part
    /// after `@` must parse as `algorithm:hex`.
    #[test]
    fn rejects_at_sign_in_non_digest_reference() {
        assert!(ImageRef::parse("ghcr.io/foo@nottag").is_err());
    }

    /// Path round-trip holds for every ref whose tag does not contain
    /// `__` — the v2-inherited encoding maps `:` to `__`, so a tag
    /// already containing `__` is the documented break point.
    #[test]
    fn round_trip_path_layout_for_non_underscore_tags() {
        for input in [
            "localhost:5000/test_repo:latest",
            "ubuntu:20.04",
            "alpine",
            "quay.io/jitesoft/alpine:sha256:6755355f801f8e3694bffb1a925786813462cea16f1ce2b0290b6a48acf2500c",
        ] {
            let r = ImageRef::parse(input).unwrap();
            let path = r.as_path();
            let parsed = ImageRef::from_path(&path).unwrap();
            assert_eq!(parsed, r, "round-trip failed for {input}");
        }
    }

    /// Tags that legitimately contain `__` collide with the legacy
    /// path encoding of `:`, so the round-trip is not lossless for
    /// that case — `from_path` decodes every `__` back to `:`, which
    /// reshapes a tag `my__tag` into the digest-shaped reference
    /// `my:tag`. The decoded reference still satisfies the OCI
    /// digest grammar, so this is silent corruption rather than an
    /// error, which is exactly why [`ImageRef::as_path`]'s doc
    /// comment scopes the round-trip claim to refs whose tag does
    /// not contain `__`. OMMX-generated refs never use `__` in tags,
    /// so the v2 → v3 legacy import path is unaffected.
    #[test]
    fn path_layout_round_trip_is_lossy_for_double_underscore_tags() {
        let r = ImageRef::parse("example.com/foo:my__tag").unwrap();
        let path = r.as_path();
        let decoded = ImageRef::from_path(&path).expect("decoded ref shape is still valid");
        assert_ne!(
            decoded, r,
            "round-trip must visibly differ so the lossy case is testable",
        );
        assert_eq!(decoded.reference(), "my:tag");
    }

    #[test]
    fn serde_round_trips_through_string() {
        let r = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v1").unwrap();
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"ghcr.io/jij-inc/ommx/demo:v1\"");
        let r2: ImageRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }
}
