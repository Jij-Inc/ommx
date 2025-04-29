use super::*;
use anyhow::bail;
use serde::{ser::*, Serialize};
use std::{collections::BTreeSet, ops::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryIds(BTreeSet<u64>);

impl Ord for BinaryIds {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = &self.0;
        let b = &other.0;
        if a.len() != b.len() {
            b.len().cmp(&a.len())
        } else {
            a.cmp(b)
        }
    }
}

impl PartialOrd for BinaryIds {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<MonomialDyn> for BinaryIds {
    fn from(ids: MonomialDyn) -> Self {
        Self(ids.into_inner().into_iter().collect())
    }
}

impl Deref for BinaryIds {
    type Target = BTreeSet<u64>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Serialize for BinaryIds {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(self.0.len())?;
        for id in &self.0 {
            tup.serialize_element(id)?;
        }
        tup.end()
    }
}

/// ID pair for QUBO problems
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct BinaryIdPair(pub u64, pub u64);

impl TryFrom<Vec<u64>> for BinaryIdPair {
    type Error = anyhow::Error;
    fn try_from(mut ids: Vec<u64>) -> Result<Self, Self::Error> {
        ids.sort_unstable();
        ids.dedup();
        match &ids[..] {
            [a, b] if a <= b => Ok(Self(*a, *b)),
            [a, b] => Ok(Self(*b, *a)),
            // For binary variable $x$, $x^2 = x$
            [a] => Ok(Self(*a, *a)),
            _ => bail!("Invalid ID for QUBO: {ids:?}"),
        }
    }
}

impl TryFrom<MonomialDyn> for BinaryIdPair {
    type Error = anyhow::Error;
    fn try_from(ids: MonomialDyn) -> Result<Self, Self::Error> {
        Self::try_from(ids.into_inner())
    }
}

impl TryFrom<BinaryIds> for BinaryIdPair {
    type Error = anyhow::Error;
    fn try_from(ids: BinaryIds) -> Result<Self, Self::Error> {
        Self::try_from(ids.0.into_iter().collect::<Vec<u64>>())
    }
}

impl Serialize for BinaryIdPair {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tup = serializer.serialize_tuple(2)?;
        tup.serialize_element(&self.0)?;
        tup.serialize_element(&self.1)?;
        tup.end()
    }
}
