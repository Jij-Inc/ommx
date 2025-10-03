//! Dataset for mathematical programming problems distributed as OMMX Artifact.
//!
//! # MIPLIB 2017
//!
//! MIPLIB 2017 is a collection of mixed-integer programming (MIP) instances.
//!
//! ```rust
//! use ommx::dataset::miplib2017;
//!
//! // Get an instance and its annotations
//! let (instance, annotation) = miplib2017::load("air05").unwrap();
//!
//! // Metadata of the MIPLIB 2017 instance is stored in the annotation
//! assert_eq!(annotation.title().unwrap(), "air05");
//! assert_eq!(annotation.authors().unwrap().next(), Some("G. Astfalk"));
//! assert_eq!(annotation.license().unwrap(), "CC-BY-SA-4.0");
//! assert_eq!(annotation.dataset().unwrap(), "MIPLIB2017");
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
//! let annotation = annotations.get("QPLIB_0018").unwrap();
//!
//! // Metadata is stored in the annotation
//! assert_eq!(annotation.title().unwrap(), "QPLIB_0018");
//! assert_eq!(annotation.dataset().unwrap(), "QPLIB");
//! assert_eq!(annotation.get("org.ommx.qplib.nvars").unwrap(), "50");
//! ```

pub mod miplib2017;
pub mod qplib;
