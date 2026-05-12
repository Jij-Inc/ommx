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
//! - [`archive`] — `.ommx` OCI archive ingest via the native v3
//!   tar streamer. Walks the archive entries once, writes each
//!   `blobs/sha256/<digest>` blob straight into [`super::FileBlobStore`]
//!   (which recomputes sha256 and asserts it matches the tar path),
//!   buffers `oci-layout` + `index.json` in memory for the post-pass
//!   parse, and emits a single SQLite transaction that publishes the
//!   manifest + ref. No on-disk OCI Image Layout cache is produced —
//!   SQLite + `FileBlobStore` are the sole post-import home.
//! - [`remote`] — remote OCI registry pull via [`oci_client`]. A
//!   pre-pull SQLite check short-circuits the network fetch when the
//!   registry already resolves the requested ref, replacing the v2-era
//!   "skip if legacy dir exists" optimisation with a check against the
//!   canonical ref store. Feature-gated behind `remote-artifact` since
//!   it is the only network-touching path in `local_registry`.

pub mod archive;
pub mod legacy;
pub mod oci_dir;
#[cfg(feature = "remote-artifact")]
pub mod remote;
