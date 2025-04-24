use crate::VariableID;

/// The ID of the variable.
///
/// Invariants
/// -----------
/// - `smaller` ID is less than or equal to `larger` ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VariableIDPair {
    smaller: VariableID,
    larger: VariableID,
}

impl VariableIDPair {
    pub fn new(id1: VariableID, id2: VariableID) -> Self {
        if id1 <= id2 {
            return Self {
                smaller: id1,
                larger: id2,
            };
        } else {
            return Self {
                smaller: id2,
                larger: id1,
            };
        }
    }

    pub fn get_smaller(&self) -> VariableID {
        self.smaller
    }

    pub fn get_larger(&self) -> VariableID {
        self.larger
    }
}
