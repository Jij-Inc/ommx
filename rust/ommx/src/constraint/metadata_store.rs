use crate::constraint::{ConstraintMetadata, Provenance};
use crate::constraint_type::IDType;
use crate::logical_memory::LogicalMemoryProfile;
use fnv::FnvHashMap;
use std::sync::OnceLock;

fn empty_parameters() -> &'static FnvHashMap<String, String> {
    static EMPTY: OnceLock<FnvHashMap<String, String>> = OnceLock::new();
    EMPTY.get_or_init(FnvHashMap::default)
}

/// ID-keyed Struct-of-Arrays storage for constraint metadata.
///
/// One [`ConstraintMetadataStore`] sits at the collection layer (one per
/// [`ConstraintCollection`](crate::ConstraintCollection),
/// [`EvaluatedCollection`](crate::EvaluatedCollection), and
/// [`SampledCollection`](crate::SampledCollection)) and holds the metadata for
/// every constraint in that collection. The type is generic over the ID type
/// so all four constraint kinds (regular / indicator / one-hot / SOS1) share
/// one implementation.
///
/// Per-field storage uses sparse [`FnvHashMap`]s — the absence of an ID from a
/// given field map encodes "no value set". Access goes through per-field
/// borrowing getters that hide the sparse representation behind a uniform
/// "missing = neutral" view (`Option<&str>` for textual fields, an empty slice
/// or shared empty map for collection-shaped fields).
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct ConstraintMetadataStore<ID: IDType> {
    name: FnvHashMap<ID, String>,
    subscripts: FnvHashMap<ID, Vec<i64>>,
    parameters: FnvHashMap<ID, FnvHashMap<String, String>>,
    description: FnvHashMap<ID, String>,
    provenance: FnvHashMap<ID, Vec<Provenance>>,
}

impl<ID: IDType> Default for ConstraintMetadataStore<ID> {
    fn default() -> Self {
        Self {
            name: FnvHashMap::default(),
            subscripts: FnvHashMap::default(),
            parameters: FnvHashMap::default(),
            description: FnvHashMap::default(),
            provenance: FnvHashMap::default(),
        }
    }
}

impl<ID: IDType> ConstraintMetadataStore<ID> {
    pub fn new() -> Self {
        Self::default()
    }

    // ===== Per-field borrowing getters =====

    pub fn name(&self, id: ID) -> Option<&str> {
        self.name.get(&id).map(String::as_str)
    }

    pub fn subscripts(&self, id: ID) -> &[i64] {
        self.subscripts.get(&id).map_or(&[], Vec::as_slice)
    }

    pub fn parameters(&self, id: ID) -> &FnvHashMap<String, String> {
        self.parameters
            .get(&id)
            .unwrap_or_else(|| empty_parameters())
    }

    pub fn description(&self, id: ID) -> Option<&str> {
        self.description.get(&id).map(String::as_str)
    }

    pub fn provenance(&self, id: ID) -> &[Provenance] {
        self.provenance.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Reconstruct the full metadata for a single id as an owned struct.
    pub fn collect_for(&self, id: ID) -> ConstraintMetadata {
        ConstraintMetadata {
            name: self.name.get(&id).cloned(),
            subscripts: self.subscripts.get(&id).cloned().unwrap_or_default(),
            parameters: self.parameters.get(&id).cloned().unwrap_or_default(),
            description: self.description.get(&id).cloned(),
            provenance: self.provenance.get(&id).cloned().unwrap_or_default(),
        }
    }

    // ===== Setters (write-through to the SoA store) =====

    pub fn set_name(&mut self, id: ID, name: impl Into<String>) {
        self.name.insert(id, name.into());
    }

    pub fn clear_name(&mut self, id: ID) {
        self.name.remove(&id);
    }

    pub fn set_subscripts(&mut self, id: ID, s: impl Into<Vec<i64>>) {
        let s = s.into();
        if s.is_empty() {
            self.subscripts.remove(&id);
        } else {
            self.subscripts.insert(id, s);
        }
    }

    pub fn push_subscript(&mut self, id: ID, value: i64) {
        self.subscripts.entry(id).or_default().push(value);
    }

    pub fn extend_subscripts(&mut self, id: ID, iter: impl IntoIterator<Item = i64>) {
        let entry = self.subscripts.entry(id).or_default();
        entry.extend(iter);
        if entry.is_empty() {
            self.subscripts.remove(&id);
        }
    }

    pub fn set_parameter(&mut self, id: ID, key: impl Into<String>, value: impl Into<String>) {
        self.parameters
            .entry(id)
            .or_default()
            .insert(key.into(), value.into());
    }

    pub fn set_parameters(&mut self, id: ID, params: FnvHashMap<String, String>) {
        if params.is_empty() {
            self.parameters.remove(&id);
        } else {
            self.parameters.insert(id, params);
        }
    }

    pub fn set_description(&mut self, id: ID, desc: impl Into<String>) {
        self.description.insert(id, desc.into());
    }

    pub fn clear_description(&mut self, id: ID) {
        self.description.remove(&id);
    }

    pub fn push_provenance(&mut self, id: ID, p: Provenance) {
        self.provenance.entry(id).or_default().push(p);
    }

    pub fn set_provenance(&mut self, id: ID, p: Vec<Provenance>) {
        if p.is_empty() {
            self.provenance.remove(&id);
        } else {
            self.provenance.insert(id, p);
        }
    }

    // ===== Bulk owned exchange with the I/O struct =====

    /// Insert the metadata of one id, replacing any existing entry.
    ///
    /// Empty metadata fields are not stored (they would be indistinguishable
    /// from "missing" anyway), keeping the maps sparse.
    pub fn insert(&mut self, id: ID, metadata: ConstraintMetadata) {
        let ConstraintMetadata {
            name,
            subscripts,
            parameters,
            description,
            provenance,
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
        if provenance.is_empty() {
            self.provenance.remove(&id);
        } else {
            self.provenance.insert(id, provenance);
        }
    }

    /// Remove and return all metadata for the given id.
    ///
    /// Returns the default (empty) [`ConstraintMetadata`] if the id was not
    /// present in any of the field maps.
    pub fn remove(&mut self, id: ID) -> ConstraintMetadata {
        ConstraintMetadata {
            name: self.name.remove(&id),
            subscripts: self.subscripts.remove(&id).unwrap_or_default(),
            parameters: self.parameters.remove(&id).unwrap_or_default(),
            description: self.description.remove(&id),
            provenance: self.provenance.remove(&id).unwrap_or_default(),
        }
    }

    /// Whether the store has any metadata recorded for the given id.
    pub fn contains(&self, id: ID) -> bool {
        self.name.contains_key(&id)
            || self.subscripts.contains_key(&id)
            || self.parameters.contains_key(&id)
            || self.description.contains_key(&id)
            || self.provenance.contains_key(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConstraintID;

    #[test]
    fn empty_store_returns_neutral_values() {
        let store: ConstraintMetadataStore<ConstraintID> = ConstraintMetadataStore::new();
        let id = ConstraintID::from(1);
        assert_eq!(store.name(id), None);
        assert!(store.subscripts(id).is_empty());
        assert!(store.parameters(id).is_empty());
        assert_eq!(store.description(id), None);
        assert!(store.provenance(id).is_empty());
        assert_eq!(store.collect_for(id), ConstraintMetadata::default());
        assert!(!store.contains(id));
    }

    #[test]
    fn insert_then_collect_round_trip() {
        let mut store: ConstraintMetadataStore<ConstraintID> = ConstraintMetadataStore::new();
        let id = ConstraintID::from(7);
        let mut params = FnvHashMap::default();
        params.insert("k".to_string(), "v".to_string());
        let metadata = ConstraintMetadata {
            name: Some("c".to_string()),
            subscripts: vec![1, 2, 3],
            parameters: params.clone(),
            description: Some("d".to_string()),
            provenance: vec![],
        };
        store.insert(id, metadata.clone());
        assert_eq!(store.name(id), Some("c"));
        assert_eq!(store.subscripts(id), &[1, 2, 3]);
        assert_eq!(store.parameters(id), &params);
        assert_eq!(store.description(id), Some("d"));
        assert!(store.provenance(id).is_empty());
        assert_eq!(store.collect_for(id), metadata);
    }

    #[test]
    fn empty_metadata_does_not_create_entries() {
        // Inserting fully-default metadata leaves the store sparse — there is
        // no way to distinguish "explicitly empty" from "never set", and the
        // sparse maps must reflect that.
        let mut store: ConstraintMetadataStore<ConstraintID> = ConstraintMetadataStore::new();
        let id = ConstraintID::from(0);
        store.insert(id, ConstraintMetadata::default());
        assert!(!store.contains(id));
    }

    #[test]
    fn remove_returns_owned_metadata_and_clears() {
        let mut store: ConstraintMetadataStore<ConstraintID> = ConstraintMetadataStore::new();
        let id = ConstraintID::from(3);
        store.set_name(id, "n");
        store.set_subscripts(id, vec![9]);
        let removed = store.remove(id);
        assert_eq!(removed.name.as_deref(), Some("n"));
        assert_eq!(removed.subscripts, vec![9]);
        assert!(!store.contains(id));
    }

    #[test]
    fn setters_write_through() {
        let mut store: ConstraintMetadataStore<ConstraintID> = ConstraintMetadataStore::new();
        let id = ConstraintID::from(11);
        store.set_name(id, "demand");
        store.set_description(id, "desc");
        store.set_parameter(id, "k1", "v1");
        store.set_parameter(id, "k2", "v2");
        store.push_subscript(id, 1);
        store.push_subscript(id, 2);
        assert_eq!(store.name(id), Some("demand"));
        assert_eq!(store.description(id), Some("desc"));
        assert_eq!(store.subscripts(id), &[1, 2]);
        assert_eq!(store.parameters(id).len(), 2);
    }
}
