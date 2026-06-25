use crate::constraint::{ConstraintContext, Provenance};
use crate::constraint_type::IDType;
use crate::logical_memory::LogicalMemoryProfile;
use crate::ModelingLabelStore;
use fnv::FnvHashMap;
use std::collections::BTreeSet;

/// ID-keyed storage for constraint labels and transformation provenance.
///
/// The human-facing modeling context (`name`, `subscripts`, `parameters`, and
/// `description`) is stored as a [`ModelingLabelStore`]. Constraint
/// transformation lineage is a separate constraint-only sidecar, not part of
/// the modeling label.
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct ConstraintContextStore<ID: IDType> {
    labels: ModelingLabelStore<ID>,
    provenance: FnvHashMap<ID, Vec<Provenance>>,
}

impl<ID: IDType> Default for ConstraintContextStore<ID> {
    fn default() -> Self {
        Self {
            labels: ModelingLabelStore::default(),
            provenance: FnvHashMap::default(),
        }
    }
}

impl<ID: IDType> ConstraintContextStore<ID> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn labels(&self) -> &ModelingLabelStore<ID> {
        &self.labels
    }

    pub fn labels_mut(&mut self) -> &mut ModelingLabelStore<ID> {
        &mut self.labels
    }

    pub fn name(&self, id: ID) -> Option<&str> {
        self.labels.name(id)
    }

    pub fn subscripts(&self, id: ID) -> &[i64] {
        self.labels.subscripts(id)
    }

    pub fn parameters(&self, id: ID) -> &FnvHashMap<String, String> {
        self.labels.parameters(id)
    }

    pub fn description(&self, id: ID) -> Option<&str> {
        self.labels.description(id)
    }

    pub fn provenance(&self, id: ID) -> &[Provenance] {
        self.provenance.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Reconstruct the full constraint context transfer object for one id.
    pub fn collect_for(&self, id: ID) -> ConstraintContext {
        ConstraintContext {
            label: self.labels.collect_for(id),
            provenance: self.provenance.get(&id).cloned().unwrap_or_default(),
        }
    }

    pub fn set_name(&mut self, id: ID, name: impl Into<String>) {
        self.labels.set_name(id, name);
    }

    pub fn clear_name(&mut self, id: ID) {
        self.labels.clear_name(id);
    }

    pub fn set_subscripts(&mut self, id: ID, s: impl Into<Vec<i64>>) {
        self.labels.set_subscripts(id, s);
    }

    pub fn push_subscript(&mut self, id: ID, value: i64) {
        self.labels.push_subscript(id, value);
    }

    pub fn extend_subscripts(&mut self, id: ID, iter: impl IntoIterator<Item = i64>) {
        self.labels.extend_subscripts(id, iter);
    }

    pub fn set_parameter(&mut self, id: ID, key: impl Into<String>, value: impl Into<String>) {
        self.labels.set_parameter(id, key, value);
    }

    pub fn set_parameters(&mut self, id: ID, params: FnvHashMap<String, String>) {
        self.labels.set_parameters(id, params);
    }

    pub fn set_description(&mut self, id: ID, desc: impl Into<String>) {
        self.labels.set_description(id, desc);
    }

    pub fn clear_description(&mut self, id: ID) {
        self.labels.clear_description(id);
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

    /// Insert the label and provenance for one id, replacing existing entries.
    pub fn insert(&mut self, id: ID, context: ConstraintContext) {
        let ConstraintContext { label, provenance } = context;
        self.labels.insert(id, label);
        self.set_provenance(id, provenance);
    }

    /// Remove and return all label/provenance data for the given id.
    pub fn remove(&mut self, id: ID) -> ConstraintContext {
        ConstraintContext {
            label: self.labels.remove(id),
            provenance: self.provenance.remove(&id).unwrap_or_default(),
        }
    }

    pub fn contains(&self, id: ID) -> bool {
        self.labels.contains(id) || self.provenance.contains_key(&id)
    }

    pub fn ids(&self) -> BTreeSet<ID> {
        self.labels
            .ids()
            .into_iter()
            .chain(self.provenance.keys().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConstraintID, ModelingLabel};

    #[test]
    fn empty_store_returns_neutral_values() {
        let store: ConstraintContextStore<ConstraintID> = ConstraintContextStore::new();
        let id = ConstraintID::from(1);
        assert_eq!(store.name(id), None);
        assert!(store.subscripts(id).is_empty());
        assert!(store.parameters(id).is_empty());
        assert_eq!(store.description(id), None);
        assert!(store.provenance(id).is_empty());
        assert_eq!(store.collect_for(id), ConstraintContext::default());
        assert!(!store.contains(id));
        assert!(store.ids().is_empty());
    }

    #[test]
    fn insert_then_collect_round_trip() {
        let mut store: ConstraintContextStore<ConstraintID> = ConstraintContextStore::new();
        let id = ConstraintID::from(7);
        let mut params = FnvHashMap::default();
        params.insert("k".to_string(), "v".to_string());
        let context = ConstraintContext {
            label: ModelingLabel {
                name: Some("c".to_string()),
                subscripts: vec![1, 2, 3],
                parameters: params.clone(),
                description: Some("d".to_string()),
            },
            provenance: vec![],
        };
        store.insert(id, context.clone());
        assert_eq!(store.name(id), Some("c"));
        assert_eq!(store.subscripts(id), &[1, 2, 3]);
        assert_eq!(store.parameters(id), &params);
        assert_eq!(store.description(id), Some("d"));
        assert!(store.provenance(id).is_empty());
        assert_eq!(store.collect_for(id), context);
        assert!(store.ids().contains(&id));
    }

    #[test]
    fn empty_context_does_not_create_entries() {
        let mut store: ConstraintContextStore<ConstraintID> = ConstraintContextStore::new();
        let id = ConstraintID::from(0);
        store.insert(id, ConstraintContext::default());
        assert!(!store.contains(id));
        assert!(store.ids().is_empty());
    }
}
