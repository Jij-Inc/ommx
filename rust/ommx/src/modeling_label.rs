use crate::constraint_type::IDType;
use crate::logical_memory::LogicalMemoryProfile;
use fnv::FnvHashMap;
use std::collections::BTreeSet;
use std::sync::OnceLock;

fn empty_parameters() -> &'static FnvHashMap<String, String> {
    static EMPTY: OnceLock<FnvHashMap<String, String>> = OnceLock::new();
    EMPTY.get_or_init(FnvHashMap::default)
}

/// Structured label that preserves the element's original modeling context.
///
/// This represents the mathematical-model notation users authored before the
/// model was lowered into OMMX. For example, a variable written as `x[i, j]`
/// can be represented as `name = "x"` with `subscripts = [i, j]`; a family of
/// constraints such as `flow_limit[place]` can use `name = "flow limit"` with
/// `parameters = {"place": place}`.
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct ModelingLabel {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

/// ID-keyed Struct-of-Arrays storage for [`ModelingLabel`].
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct ModelingLabelStore<ID: IDType> {
    name: FnvHashMap<ID, String>,
    subscripts: FnvHashMap<ID, Vec<i64>>,
    parameters: FnvHashMap<ID, FnvHashMap<String, String>>,
    description: FnvHashMap<ID, String>,
}

impl<ID: IDType> Default for ModelingLabelStore<ID> {
    fn default() -> Self {
        Self {
            name: FnvHashMap::default(),
            subscripts: FnvHashMap::default(),
            parameters: FnvHashMap::default(),
            description: FnvHashMap::default(),
        }
    }
}

impl<ID: IDType> ModelingLabelStore<ID> {
    pub fn new() -> Self {
        Self::default()
    }

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

    pub fn collect_for(&self, id: ID) -> ModelingLabel {
        ModelingLabel {
            name: self.name.get(&id).cloned(),
            subscripts: self.subscripts.get(&id).cloned().unwrap_or_default(),
            parameters: self.parameters.get(&id).cloned().unwrap_or_default(),
            description: self.description.get(&id).cloned(),
        }
    }

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

    pub fn insert(&mut self, id: ID, label: ModelingLabel) {
        let ModelingLabel {
            name,
            subscripts,
            parameters,
            description,
        } = label;
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

    pub fn remove(&mut self, id: ID) -> ModelingLabel {
        ModelingLabel {
            name: self.name.remove(&id),
            subscripts: self.subscripts.remove(&id).unwrap_or_default(),
            parameters: self.parameters.remove(&id).unwrap_or_default(),
            description: self.description.remove(&id),
        }
    }

    pub fn contains(&self, id: ID) -> bool {
        self.name.contains_key(&id)
            || self.subscripts.contains_key(&id)
            || self.parameters.contains_key(&id)
            || self.description.contains_key(&id)
    }

    pub fn ids(&self) -> BTreeSet<ID> {
        self.name
            .keys()
            .chain(self.subscripts.keys())
            .chain(self.parameters.keys())
            .chain(self.description.keys())
            .copied()
            .collect()
    }
}

/// Validate that every label ID is owned by the enclosing top-level object.
pub(crate) fn validate_modeling_label_ids<ID: IDType>(
    store: &ModelingLabelStore<ID>,
    owned_ids: &BTreeSet<ID>,
    owner_name: &str,
) -> crate::Result<()> {
    if let Some(id) = store.ids().into_iter().find(|id| !owned_ids.contains(id)) {
        crate::bail!(
            { ?id },
            "Modeling label references unknown {owner_name} ID {id:?}",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VariableID;

    #[test]
    fn empty_store_returns_neutral_values() {
        let store = ModelingLabelStore::<VariableID>::new();
        let id = VariableID::from(1);
        assert_eq!(store.name(id), None);
        assert!(store.subscripts(id).is_empty());
        assert!(store.parameters(id).is_empty());
        assert_eq!(store.description(id), None);
        assert_eq!(store.collect_for(id), ModelingLabel::default());
        assert!(!store.contains(id));
        assert!(store.ids().is_empty());
    }

    #[test]
    fn insert_then_collect_round_trip() {
        let mut store = ModelingLabelStore::<VariableID>::new();
        let id = VariableID::from(42);
        let mut params = FnvHashMap::default();
        params.insert("k".into(), "v".into());
        let label = ModelingLabel {
            name: Some("x".to_string()),
            subscripts: vec![0, 1],
            parameters: params,
            description: Some("d".to_string()),
        };
        store.insert(id, label.clone());
        assert_eq!(store.collect_for(id), label);
        assert!(store.ids().contains(&id));
    }

    #[test]
    fn empty_label_does_not_create_entries() {
        let mut store = ModelingLabelStore::<VariableID>::new();
        let id = VariableID::from(0);
        store.insert(id, ModelingLabel::default());
        assert!(!store.contains(id));
        assert!(store.ids().is_empty());
    }

    #[test]
    fn setters_write_through() {
        let mut store = ModelingLabelStore::<VariableID>::new();
        let id = VariableID::from(11);
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
        assert!(store.ids().contains(&id));
    }

    #[test]
    fn remove_returns_owned_label_and_clears() {
        let mut store = ModelingLabelStore::<VariableID>::new();
        let id = VariableID::from(3);
        store.set_name(id, "n");
        store.set_subscripts(id, vec![9]);

        let removed = store.remove(id);

        assert_eq!(removed.name.as_deref(), Some("n"));
        assert_eq!(removed.subscripts, vec![9]);
        assert!(!store.contains(id));
    }
}
