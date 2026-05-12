//! `.ommx` OCI archive builder backed by the user's SQLite Local Registry.
//!
//! Step F (§12.4) rewrote `ArchiveArtifactBuilder` as a thin
//! convenience wrapper around [`LocalArtifactBuilder`] that publishes
//! the artifact into the **user's persistent SQLite Local Registry**
//! and then calls [`LocalArtifact::save`] to write the `.ommx` archive
//! file. The archive is purely an exchange-format export of the
//! registry-resident artifact; there is no transient or anonymous
//! archive path in v3.
//!
//! All blob writes go through the v3 native code path
//! ([`crate::artifact::save`]'s native tar writer) and the produced
//! archive is a strict OCI Image Layout (`oci-layout`, `index.json`,
//! `blobs/<algorithm>/<encoded>`) — the v2-era `OciArchiveBuilder` is
//! no longer involved.

use crate::artifact::{
    local_registry::{LocalRegistry, RefConflictPolicy},
    media_types, Config, InstanceAnnotations, LocalArtifact, LocalArtifactBuilder,
    ParametricInstanceAnnotations, SampleSetAnnotations, SolutionAnnotations,
};
use crate::v1;
use anyhow::{Context, Result};
use ocipkg::ImageName;
use prost::Message;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Build a `.ommx` OCI archive file backed by the user's SQLite Local
/// Registry. v3-native build into the registry alone uses
/// [`LocalArtifactBuilder`] instead; this type is the archive-output
/// convenience that also leaves a registry entry behind, so the
/// resulting artifact is addressable by ref name afterward. The
/// previous v2 surface that used `OciArchiveBuilder` is gone.
pub struct ArchiveArtifactBuilder {
    inner: LocalArtifactBuilder,
    output_path: PathBuf,
    registry: Arc<LocalRegistry>,
}

impl ArchiveArtifactBuilder {
    /// Build an archive at `path` under the given `image_name`. The
    /// image name is written into the archive's `index.json` as the
    /// `org.opencontainers.image.ref.name` annotation, matching what
    /// [`LocalArtifact::save`] produces, **and** is published into the
    /// user's persistent SQLite Local Registry as a side effect of
    /// `build()`. v3 does not support `ref.name`-absent archives — every
    /// archive carries a ref so the SQLite Local Registry can address
    /// it on import / export.
    pub fn new_archive(path: PathBuf, image_name: ImageName) -> Result<Self> {
        if path.exists() {
            crate::bail!("Archive output file already exists: {}", path.display());
        }
        let registry = Arc::new(LocalRegistry::open_default().with_context(|| {
            "Failed to open the default SQLite Local Registry for archive build"
        })?);
        let inner = LocalArtifactBuilder::new(image_name);
        Ok(Self {
            inner,
            output_path: path,
            registry,
        })
    }

    /// Build an archive at `path` without a caller-supplied image
    /// name. UX shortcut for "I just want to share a `.ommx` file and
    /// don't want to invent a ref". A per-call synthetic placeholder
    /// of the form `local.ommx/anonymous-<UTC-timestamp>:tmp` is
    /// generated and written into the archive's `index.json` as
    /// `org.opencontainers.image.ref.name`, AND published into the
    /// user's persistent SQLite Local Registry alongside the named
    /// builds — the registry is the canonical store in v3, so anonymous
    /// archives must be addressable there too. `LocalArtifact::image_name`
    /// on the returned handle surfaces the synthesized name (in pre-v3
    /// the equivalent path produced `image_name == None`).
    ///
    /// The placeholder hostname `local.ommx` is intentionally not a
    /// real registry so a synthetic ref cannot accidentally resolve
    /// against a remote registry; the timestamp suffix keeps two
    /// anonymous builds in the same SQLite registry from colliding
    /// **and** lets a user inspecting `ommx artifact prune-anonymous`
    /// output later tell at a glance when each entry was created.
    /// Consumers who want a stable, human-readable name should use
    /// [`Self::new_archive`] instead. `ommx artifact prune-anonymous`
    /// bulk-deletes accumulated synthetic refs.
    pub fn new_archive_unnamed(path: PathBuf) -> Result<Self> {
        Self::new_archive(path, anonymous_archive_image_name()?)
    }

    /// Create a temporary archive at the OS temp dir under a random
    /// `ttl.sh` image name. Insecure; for tests only.
    pub fn temp_archive() -> Result<Self> {
        let id = Uuid::new_v4();
        let path = std::env::temp_dir().join(format!("ommx-{id}.ommx"));
        let image_name = ImageName::parse(&format!("ttl.sh/{id}:1h"))?;
        Self::new_archive(path, image_name)
    }

    pub fn add_instance(
        &mut self,
        instance: v1::Instance,
        annotations: InstanceAnnotations,
    ) -> Result<()> {
        self.inner
            .add_layer_bytes(
                media_types::v1_instance(),
                instance.encode_to_vec(),
                annotations.into(),
            )
            .map(|_| ())
    }

    pub fn add_solution(
        &mut self,
        solution: v1::State,
        annotations: SolutionAnnotations,
    ) -> Result<()> {
        self.inner
            .add_layer_bytes(
                media_types::v1_solution(),
                solution.encode_to_vec(),
                annotations.into(),
            )
            .map(|_| ())
    }

    pub fn add_parametric_instance(
        &mut self,
        instance: v1::ParametricInstance,
        annotations: ParametricInstanceAnnotations,
    ) -> Result<()> {
        self.inner
            .add_layer_bytes(
                media_types::v1_parametric_instance(),
                instance.encode_to_vec(),
                annotations.into(),
            )
            .map(|_| ())
    }

    pub fn add_sample_set(
        &mut self,
        sample_set: v1::SampleSet,
        annotations: SampleSetAnnotations,
    ) -> Result<()> {
        self.inner
            .add_layer_bytes(
                media_types::v1_sample_set(),
                sample_set.encode_to_vec(),
                annotations.into(),
            )
            .map(|_| ())
    }

    /// Raw layer addition. Bytes are passed verbatim; the descriptor
    /// is computed inside [`LocalArtifactBuilder::add_layer_bytes`].
    pub fn add_layer(
        &mut self,
        media_type: oci_spec::image::MediaType,
        bytes: &[u8],
        annotations: HashMap<String, String>,
    ) -> Result<oci_spec::image::Descriptor> {
        self.inner
            .add_layer_bytes(media_type, bytes.to_vec(), annotations)
    }

    /// `add_config` is no longer supported on the archive path. The
    /// v2 SDK accepted an OMMX-specific config blob; v3 sets the
    /// config descriptor to the OCI 1.1 empty config + carries OMMX
    /// state in `artifactType`. `Config` is therefore best embedded
    /// as a regular layer if required by downstream consumers.
    #[deprecated(
        note = "v3 builds always emit the OCI 1.1 empty config; embed config-like state \
                in a layer via `add_layer` instead"
    )]
    pub fn add_config(&mut self, _config: Config) -> Result<()> {
        crate::bail!(
            "ArchiveArtifactBuilder::add_config is not supported in v3; the OMMX manifest \
             carries `artifactType` and uses the OCI 1.1 empty config blob. If a custom \
             config payload is needed, add it as a layer."
        )
    }

    pub fn add_annotation(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.inner.add_annotation(key, value);
    }

    pub fn add_source(&mut self, url: &url::Url) {
        self.inner.add_source(url);
    }

    pub fn add_description(&mut self, description: String) {
        self.inner
            .add_annotation("org.opencontainers.image.description", description);
    }

    pub fn build(self) -> Result<LocalArtifact> {
        let local = self
            .inner
            .build_in_registry(self.registry, RefConflictPolicy::Replace)?;
        local.save(&self.output_path)?;
        // `local` points into the user's persistent SQLite registry, so
        // the artifact is reachable by ref name after `build()` without
        // an additional `Artifact.load(...)` round-trip. The archive
        // file at `output_path` is the exchange-format export.
        Ok(local)
    }
}

/// Prefix shared by every synthetic ref the anonymous-archive UX
/// shortcut generates. `ommx artifact prune-anonymous` deletes refs
/// whose SQLite `name` column starts with this string.
pub const ANONYMOUS_ARCHIVE_REF_NAME_PREFIX: &str = "local.ommx/anonymous-";

/// Generate a synthetic `org.opencontainers.image.ref.name` for the
/// anonymous-archive UX shortcut. The suffix is a UTC timestamp
/// (`YYYY-MM-DD-HH-MM-SS-<nanos>`) so a user inspecting their SQLite
/// Local Registry months later can tell at a glance when the entry
/// was created. Nanosecond precision keeps two anonymous builds in
/// the same second from colliding; if they ever did, the underlying
/// builder uses `RefConflictPolicy::Replace`, which overwrites the
/// older entry rather than failing.
///
/// The hostname segment `local.ommx` is deliberately a non-registry
/// placeholder so a synthetic ref cannot collide with or resolve
/// against a real remote registry. Used by
/// [`ArchiveArtifactBuilder::new_archive_unnamed`] and the Python
/// equivalent `ArtifactBuilder.new_archive_unnamed`.
pub fn anonymous_archive_image_name() -> Result<ImageName> {
    let stamp = chrono::Utc::now().format("%Y-%m-%d-%H-%M-%S-%9f");
    ImageName::parse(&format!("{ANONYMOUS_ARCHIVE_REF_NAME_PREFIX}{stamp}:tmp")).with_context(
        || format!("Failed to synthesise placeholder image name for unnamed archive: {stamp}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `chrono::Utc::now().format("%Y-%m-%d-%H-%M-%S-%9f")` produces a
    /// string whose every component is alphanumeric with `-` separators
    /// — the OCI distribution spec accepts this as a name segment. A
    /// regression that swapped the format to include `:` or `.` would
    /// break `ImageName::parse` here.
    #[test]
    fn anonymous_image_name_parses() {
        let name = anonymous_archive_image_name().expect("synthetic ref must parse");
        let s = name.to_string();
        assert!(
            s.starts_with(ANONYMOUS_ARCHIVE_REF_NAME_PREFIX),
            "synthetic ref `{s}` must start with `{ANONYMOUS_ARCHIVE_REF_NAME_PREFIX}`",
        );
        assert!(
            s.ends_with(":tmp"),
            "synthetic ref `{s}` must end with `:tmp`"
        );
    }
}
