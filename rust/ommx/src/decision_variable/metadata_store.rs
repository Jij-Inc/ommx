use crate::decision_variable::DecisionVariableMetadata;
use crate::logical_memory::LogicalMemoryProfile;
use crate::VariableID;
use fnv::FnvHashMap;
use std::sync::OnceLock;

fn empty_parameters() -> &'static FnvHashMap<String, String> {
    static EMPTY: OnceLock<FnvHashMap<String, String>> = OnceLock::new();
    EMPTY.get_or_init(FnvHashMap::default)
}

/// ID-keyed Struct-of-Arrays storage for decision-variable metadata.
///
/// Sibling type to [`ConstraintMetadataStore`](crate::ConstraintMetadataStore),
/// living next to the [`Instance`](crate::Instance)'s
/// `decision_variables: BTreeMap<VariableID, DecisionVariable>` map. There is
/// no analogous `DecisionVariableCollection` to hang the store off of, so it
/// is a sibling field on `Instance` (and `ParametricInstance`).
///
/// Variables have no `provenance` field — that is constraint-specific, so the
/// store omits it.
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct VariableMetadataStore {
    name: FnvHashMap<VariableID, String>,
    subscripts: FnvHashMap<VariableID, Vec<i64>>,
    parameters: FnvHashMap<VariableID, FnvHashMap<String, String>>,
    description: FnvHashMap<VariableID, String>,
}

impl VariableMetadataStore {
    pub fn new() -> Self {
        Self::default()
    }

    // ===== Per-field borrowing getters =====

    pub fn name(&self, id: VariableID) -> Option<&str> {
        self.name.get(&id).map(String::as_str)
    }

    pub fn subscripts(&self, id: VariableID) -> &[i64] {
        self.subscripts.get(&id).map_or(&[], Vec::as_slice)
    }

    pub fn parameters(&self, id: VariableID) -> &FnvHashMap<String, String> {
        self.parameters
            .get(&id)
            .unwrap_or_else(|| empty_parameters())
    }

    pub fn description(&self, id: VariableID) -> Option<&str> {
        self.description.get(&id).map(String::as_str)
    }

    /// Reconstruct the full metadata for a single id as an owned struct.
    pub fn collect_for(&self, id: VariableID) -> DecisionVariableMetadata {
        DecisionVariableMetadata {
            name: self.name.get(&id).cloned(),
            subscripts: self.subscripts.get(&id).cloned().unwrap_or_default(),
            parameters: self.parameters.get(&id).cloned().unwrap_or_default(),
            description: self.description.get(&id).cloned(),
        }
    }

    // ===== Setters =====

    pub fn set_name(&mut self, id: VariableID, name: impl Into<String>) {
        self.name.insert(id, name.into());
    }

    pub fn clear_name(&mut self, id: VariableID) {
        self.name.remove(&id);
    }

    pub fn set_subscripts(&mut self, id: VariableID, s: impl Into<Vec<i64>>) {
        let s = s.into();
        if s.is_empty() {
            self.subscripts.remove(&id);
        } else {
            self.subscripts.insert(id, s);
        }
    }

    pub fn push_subscript(&mut self, id: VariableID, value: i64) {
        self.subscripts.entry(id).or_default().push(value);
    }

    pub fn extend_subscripts(&mut self, id: VariableID, iter: impl IntoIterator<Item = i64>) {
        let entry = self.subscripts.entry(id).or_default();
        entry.extend(iter);
        if entry.is_empty() {
            self.subscripts.remove(&id);
        }
    }

    pub fn set_parameter(
        &mut self,
        id: VariableID,
        key: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.parameters
            .entry(id)
            .or_default()
            .insert(key.into(), value.into());
    }

    pub fn set_parameters(&mut self, id: VariableID, params: FnvHashMap<String, String>) {
        if params.is_empty() {
            self.parameters.remove(&id);
        } else {
            self.parameters.insert(id, params);
        }
    }

    pub fn set_description(&mut self, id: VariableID, desc: impl Into<String>) {
        self.description.insert(id, desc.into());
    }

    pub fn clear_description(&mut self, id: VariableID) {
        self.description.remove(&id);
    }

    // ===== Bulk owned exchange =====

    /// Insert metadata for one id, replacing any existing entry. Empty fields
    /// are not stored, keeping the maps sparse.
    pub fn insert(&mut self, id: VariableID, metadata: DecisionVariableMetadata) {
        let DecisionVariableMetadata {
            name,
            subscripts,
            parameters,
            description,
        } = metadata;
        match name {
            Some(n) => self.name.insert(id, n),
            None => self.name.remove(&id),
        };
        if subscripts.is_empty() {
            self.subscripts.remove(&id);
        } else {
            self.subscripts.insert(id, subscripts);
        }
        if parameters.is_empty() {
            self.parameters.remove(&id);
        } else {
            self.parameters.insert(id, parameters);
        }
        match description {
            Some(d) => self.description.insert(id, d),
            None => self.description.remove(&id),
        };
    }

    pub fn remove(&mut self, id: VariableID) -> DecisionVariableMetadata {
        DecisionVariableMetadata {
            name: self.name.remove(&id),
            subscripts: self.subscripts.remove(&id).unwrap_or_default(),
            parameters: self.parameters.remove(&id).unwrap_or_default(),
            description: self.description.remove(&id),
        }
    }

    pub fn contains(&self, id: VariableID) -> bool {
        self.name.contains_key(&id)
            || self.subscripts.contains_key(&id)
            || self.parameters.contains_key(&id)
            || self.description.contains_key(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_store_returns_neutral_values() {
        let store = VariableMetadataStore::new();
        let id = VariableID::from(1);
        assert_eq!(store.name(id), None);
        assert!(store.subscripts(id).is_empty());
        assert!(store.parameters(id).is_empty());
        assert_eq!(store.description(id), None);
        assert_eq!(store.collect_for(id), DecisionVariableMetadata::default());
        assert!(!store.contains(id));
    }

    #[test]
    fn insert_then_collect_round_trip() {
        let mut store = VariableMetadataStore::new();
        let id = VariableID::from(42);
        let mut params = FnvHashMap::default();
        params.insert("k".into(), "v".into());
        let metadata = DecisionVariableMetadata {
            name: Some("x".to_string()),
            subscripts: vec![0, 1],
            parameters: params,
            description: Some("d".to_string()),
        };
        store.insert(id, metadata.clone());
        assert_eq!(store.collect_for(id), metadata);
    }

    #[test]
    fn empty_metadata_does_not_create_entries() {
        let mut store = VariableMetadataStore::new();
        let id = VariableID::from(0);
        store.insert(id, DecisionVariableMetadata::default());
        assert!(!store.contains(id));
    }
}
