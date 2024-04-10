//! Store messages as OCI archive
//!
//! Why OCI archive?
//! -----------------
//! OCI archive is a tar archive of [OCI image layout](https://github.com/opencontainers/image-spec/blob/v1.1.0/image-layout.md).
//! This is used, for example, in [`podman save`](https://docs.podman.io/en/latest/markdown/podman-save.1.html) to store the container image as a single file.
//! This is a good choice to store OMMX messages because:
//! - Since the data on disc lives longer than the process or even applications, we need to store metadata of message including the type of the message,
//!   i.e. `ommx.v1.Instance`, `ommx.v1.Solution`, or other types, who and when this message was created by which application, and so on.
//! - The file format must be support storing metadata within the file.
//! - The file must be support storing multiple messages in a single file.
//!

use crate::{error::*, v1};
use serde::*;

/// The version of schema used in the archive
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Version {
    V1,
}

/// Kind of the message stored in the archive
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    Instance,
    Solution,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Archive {
    V1Instance(v1::Instance),
    V1Solution(v1::Solution),
}

pub fn load(path: &str) -> Result<Archive> {
    dbg!(path);
    todo!()
}
