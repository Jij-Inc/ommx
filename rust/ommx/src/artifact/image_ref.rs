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
        let (hostname_part, rest) = input.split_once('/').unwrap_or((DEFAULT_HOSTNAME, input));
        let (hostname, port) = match hostname_part.split_once(':') {
            Some((host, port)) => {
                let port = port.parse::<u16>().context("Invalid port number")?;
                (host.to_string(), Some(port))
            }
            None => (hostname_part.to_string(), None),
        };
        let (name, reference) = rest.split_once(':').unwrap_or((rest, DEFAULT_TAG));
        validate_name(name)?;
        validate_reference(reference)?;
        Ok(Self {
            hostname,
            port,
            name: name.to_string(),
            reference: reference.to_string(),
        })
    }
}

impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.port {
            Some(port) => write!(
                f,
                "{}:{port}/{}:{}",
                self.hostname, self.name, self.reference
            ),
            None => write!(f, "{}/{}:{}", self.hostname, self.name, self.reference),
        }
    }
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

    #[test]
    fn accepts_digest_reference() {
        let s = "quay.io/jitesoft/alpine:sha256:6755355f801f8e3694bffb1a925786813462cea16f1ce2b0290b6a48acf2500c";
        let r = ImageRef::parse(s).unwrap();
        assert_eq!(r.name(), "jitesoft/alpine");
        assert_eq!(
            r.reference(),
            "sha256:6755355f801f8e3694bffb1a925786813462cea16f1ce2b0290b6a48acf2500c"
        );
    }

    #[test]
    fn rejects_invalid_capital_in_name() {
        assert!(ImageRef::parse("ghcr.io/Foo:v1").is_err());
    }

    #[test]
    fn rejects_invalid_tag_with_at_sign() {
        assert!(ImageRef::parse("ghcr.io/foo:my@tag").is_err());
    }

    #[test]
    fn round_trip_path_layout() {
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

    #[test]
    fn serde_round_trips_through_string() {
        let r = ImageRef::parse("ghcr.io/jij-inc/ommx/demo:v1").unwrap();
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"ghcr.io/jij-inc/ommx/demo:v1\"");
        let r2: ImageRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }
}
