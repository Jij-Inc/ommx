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

pub enum Archive {
    V1Instance(v1::Instance),
}

pub fn load(path: &str) -> Result<Archive> {
    dbg!(path);
    todo!()
}
