//! Dataset for mathematical programming problems distributed as OMMX Artifact.
//!
//! # MIPLIB 2017
//!
//! MIPLIB 2017 is a collection of mixed-integer programming (MIP) instances.
//!
//! ```rust
//! use ommx::dataset::miplib2017;
//!
//! // Get an instance whose descriptor annotations are merged into protobuf metadata
//! let instance = miplib2017::load("air05").unwrap();
//!
//! // Metadata of the MIPLIB 2017 instance is stored in the description
//! let description = instance.description.as_ref().unwrap();
//! assert_eq!(description.name.as_deref(), Some("air05"));
//! assert_eq!(description.authors.first().map(String::as_str), Some("G. Astfalk"));
//! assert_eq!(description.license.as_deref(), Some("CC-BY-SA-4.0"));
//! assert_eq!(description.dataset.as_deref(), Some("MIPLIB2017"));
//! ```
//!
//! # QPLIB
//!
//! QPLIB is a collection of quadratic programming (QP) instances.
//!
//! ```
//! use ommx::dataset::qplib;
//!
//! // Get metadata for all QPLIB instances
//! let annotations = qplib::instance_annotations();
//! let annotation = annotations.get("0018").unwrap();
//!
//! // Metadata is stored in flat annotation keys for catalogue lookup
//! assert_eq!(annotation.get("org.ommx.v1.instance.title").unwrap(), "QPLIB_0018");
//! assert_eq!(annotation.get("org.ommx.v1.instance.dataset").unwrap(), "QPLIB");
//! assert_eq!(annotation.get("org.ommx.qplib.nvars").unwrap(), "50");
//! ```

pub mod miplib2017;
pub mod qplib;
