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
//! ```rust
//! use ommx::dataset::qplib;
//!
//! // Get an instance and its annotations
//! let (instance, annotation) = qplib::load("QPLIB_0018").unwrap();
//!
//! // Metadata of the QPLIB instance is stored in the annotation
//! assert_eq!(annotation.title().unwrap(), "QPLIB_0018");
//! assert_eq!(annotation.dataset().unwrap(), "QPLIB");
//! ```

pub mod miplib2017;
pub mod qplib;
