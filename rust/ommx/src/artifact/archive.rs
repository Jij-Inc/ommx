//! `OmmxArchive` — handle to a `.ommx` OCI archive backed by a temp SQLite registry.
//!
//! Step F (§12.4) folded the ocipkg-typed `Artifact<OciArchive>` into
//! this non-generic struct. An archive is opened by importing its
//! blobs into a [`tempfile::TempDir`]-backed [`LocalRegistry`]; from
//! that point on every read forwards to the resulting [`LocalArtifact`],
//! so the archive surface and the registry surface are the same code
//! path. Push, save, layer enumeration, manifest access, and blob
//! lookup all live on `LocalArtifact` already, which means
//! `OmmxArchive` is a thin RAII guard around the temp registry rather
//! than a parallel reader implementation.
//!
//! The temp registry's filesystem state is kept alive by the
//! `Option<TempDir>` field for as long as the `OmmxArchive` value is
//! live; dropping the archive drops the tempdir, which deletes the
//! SQLite database and the staged blob bytes. Use cases that need the
//! archive to outlive the handle (e.g. publishing the same archive
//! many times) should hold the `OmmxArchive` for the full duration.
//!
//! `OmmxArchive` is the value
//! [`crate::artifact::ArchiveArtifactBuilder::build`] returns and what
//! [`Self::open`] produces when reading an existing `.ommx` file. The
//! Python binding wraps the same struct under `ArtifactInner::Archive`.

use super::{
    local_registry::{import_oci_archive, LocalRegistry},
    LocalArtifact, LocalManifest,
};
use anyhow::{Context, Result};
use oci_spec::image::Descriptor;
use std::{collections::HashMap, path::Path, sync::Arc};

/// Handle to a `.ommx` OCI archive.
///
/// Construction routes:
///
/// - [`Self::open`] — open an archive on disk. Imports the archive's
///   blobs + manifest into a private [`tempfile::TempDir`]-backed
///   [`LocalRegistry`], then exposes the result as a [`LocalArtifact`].
/// - [`Self::from_local_in_tempdir`] — internal constructor used by
///   [`crate::artifact::ArchiveArtifactBuilder`] after it has built a
///   local artifact in a tempdir-backed registry and saved the
///   archive file out. The tempdir is tied to the returned handle so
///   further reads stay valid.
///
/// Cheap operations forward to the wrapped [`LocalArtifact`]:
/// `get_manifest`, `get_blob`, `layers`, `annotations`, `subject`,
/// `image_name`, `manifest_digest`. Network and disk operations
/// (`push`, `save`) likewise forward.
pub struct OmmxArchive {
    local: LocalArtifact,
    // RAII guard: the tempdir owns the temp SQLite registry's files
    // and must live at least as long as `local`. `None` is reserved
    // for future cases where an archive view is opened over a
    // persistent registry; today every constructor sets it to `Some`.
    _tempdir: Option<tempfile::TempDir>,
}

impl OmmxArchive {
    /// Open a `.ommx` OCI archive on disk and import its contents
    /// into a private temp SQLite registry. The returned handle owns
    /// the tempdir; dropping it deletes the temp registry.
    ///
    /// The archive must carry an `org.opencontainers.image.ref.name`
    /// annotation on its `index.json` manifest descriptor — without
    /// a ref name, the v3 SQLite Local Registry has no key to address
    /// the artifact under. Archives produced by
    /// [`crate::artifact::LocalArtifact::save`] or the v3
    /// [`crate::artifact::ArchiveArtifactBuilder`] always carry the
    /// annotation; legacy v2-OMMX SDK archives also did.
    pub fn open(path: &Path) -> Result<Self> {
        let tempdir = tempfile::tempdir()
            .with_context(|| "Failed to create temp dir for OmmxArchive registry")?;
        let registry = Arc::new(LocalRegistry::open(tempdir.path()).with_context(|| {
            format!(
                "Failed to open temp Local Registry at {}",
                tempdir.path().display()
            )
        })?);
        let outcome = import_oci_archive(&registry, path)?;
        let image_name = outcome.image_name.with_context(|| {
            format!(
                "OCI archive at {} has no `org.opencontainers.image.ref.name` annotation; \
                 v3 requires a ref name to address the artifact",
                path.display()
            )
        })?;
        let local = LocalArtifact::open_in_registry(registry, image_name)?;
        Ok(Self {
            local,
            _tempdir: Some(tempdir),
        })
    }

    /// Construct an `OmmxArchive` from an already-built
    /// [`LocalArtifact`] and the tempdir that backs its
    /// [`LocalRegistry`]. Internal to
    /// [`crate::artifact::ArchiveArtifactBuilder::build`].
    pub(crate) fn from_local_in_tempdir(local: LocalArtifact, tempdir: tempfile::TempDir) -> Self {
        Self {
            local,
            _tempdir: Some(tempdir),
        }
    }

    /// The image ref this archive was opened / built under.
    pub fn image_name(&self) -> &ocipkg::ImageName {
        self.local.image_name()
    }

    /// The manifest digest (`sha256:<encoded>`) verbatim from the
    /// archive's `index.json`.
    pub fn manifest_digest(&self) -> &str {
        self.local.manifest_digest()
    }

    /// Parsed OCI image manifest. Cached after the first call (see
    /// [`LocalArtifact::get_manifest`]).
    pub fn get_manifest(&self) -> Result<&LocalManifest> {
        self.local.get_manifest()
    }

    /// All layer descriptors in manifest order.
    pub fn layers(&self) -> Result<Vec<Descriptor>> {
        self.local.layers()
    }

    /// Manifest-level annotations as a `HashMap`.
    pub fn annotations(&self) -> Result<HashMap<String, String>> {
        self.local.annotations()
    }

    /// Optional `subject` descriptor (used for OCI referrers).
    pub fn subject(&self) -> Result<Option<Descriptor>> {
        self.local.subject()
    }

    /// Read a blob by digest (`sha256:<encoded>`). The temp
    /// [`super::local_registry::FileBlobStore`] re-verifies the
    /// digest on read; a corrupted tempdir surfaces here.
    pub fn get_blob(&self, digest: &str) -> Result<Vec<u8>> {
        self.local.get_blob(digest)
    }

    /// Push this archive to its registry via the v3 native transport.
    /// The push reads blobs from the temp registry's CAS storage, so
    /// the network path is identical to the SQLite-registry-backed
    /// case.
    #[cfg(feature = "remote-artifact")]
    pub fn push(&self) -> Result<()> {
        self.local.push()
    }

    /// Pack the temp-registry-resident artifact into a fresh `.ommx`
    /// archive at `output`. Useful for re-emitting a transformed
    /// archive without keeping it in the user's default SQLite
    /// registry.
    pub fn save(&self, output: &Path) -> Result<()> {
        self.local.save(output)
    }
}
