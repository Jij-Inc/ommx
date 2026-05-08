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
//!
//! Future import sources (e.g. `.ommx` archive, remote pull) will land
//! here as additional submodules.

pub mod legacy;
pub mod oci_dir;
