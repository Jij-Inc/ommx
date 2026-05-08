//! Import paths that bring external content into the v3 Local Registry.
//!
//! The v3 Local Registry stores everything as content-addressed blobs
//! in [`super::FileBlobStore`] plus index records in
//! [`super::SqliteIndexStore`]; it does **not** store anything in OCI
//! Image Layout format. This module hosts the boundary code that reads
//! external sources in their native format and writes them through the
//! registry's [`super::LocalRegistry`] facade.
//!
//! Currently exposed sources:
//!
//! - [`oci_dir`] — a single OCI Image Layout directory (`oci-layout` +
//!   `index.json` + `blobs/`). The format is the OCI standard; the
//!   directory may have come from `oras` / `crane` / `skopeo`, from a
//!   v2 OMMX local registry path/tag entry, or from a hand-expanded
//!   `.ommx` archive. Identity-preserving: the manifest bytes and
//!   digest are stored verbatim.
//! - [`legacy`] — v2 OMMX local registry compatibility. Walks a
//!   `<root>/<image_name>/<tag>/` tree and runs the per-directory
//!   [`oci_dir`] import against each entry, aggregating outcomes into
//!   a [`legacy::LegacyImportReport`].
//! - [`archive`] — `.ommx` OCI archive ingest. **Currently a thin
//!   ocipkg wrapper**: extracts the archive into the legacy OCI dir
//!   via `Artifact::from_oci_archive(...).load()` and then funnels
//!   the result through [`oci_dir::import_oci_dir_as_ref`]. The
//!   public signature stays stable when the inner extraction is
//!   replaced with a native v3 path.
//! - [`remote`] — remote OCI registry pull. Same two-stage shape as
//!   [`archive`]: `Artifact::from_remote(...).pull()` writes the
//!   image into the legacy OCI dir, then [`oci_dir`] brings it into
//!   SQLite. Feature-gated behind `remote-artifact` since it is the
//!   only network-touching path in `local_registry`.
//!
//! [`archive`] and [`remote`] are explicitly the ocipkg-dependent
//! seam. Removing ocipkg from the SDK is a follow-up PR scoped to
//! these two modules; the rest of the `local_registry` tree is
//! already ocipkg-free aside from the shared `ImageName` /
//! `oci_spec` types.

pub mod archive;
pub mod legacy;
pub mod oci_dir;
#[cfg(feature = "remote-artifact")]
pub mod remote;
