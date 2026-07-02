use std::collections::{BTreeSet, HashMap};

use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use crate::{ModelingLabel, ModelingLabelStore, Parse, ParseError, RawParseError, VariableID};

/// Modeling label for parametric-instance parameters.
pub type ParameterLabel = ModelingLabel;

/// Per-parameter modeling-label store.
pub type ParameterLabelStore = ModelingLabelStore<VariableID>;

/// Owner of parameter IDs and their modeling labels.
///
/// Parameters share the same [`VariableID`] namespace as decision variables:
/// algebraic expressions carry only a `VariableID`, and the enclosing
/// [`crate::ParametricInstance`] decides whether each referenced ID is a
/// decision variable or a parameter. For that reason OMMX intentionally does
/// not introduce a separate `ParameterID` type.
///
/// `ParameterTable` owns the parameter ID universe and the
/// [`ParameterLabelStore`] sidecar. Parameter values are not stored here; they
/// are supplied later to [`crate::ParametricInstance::with_parameters`].
///
/// Mathematically, this table is only the parameter key set `P` plus modeling
/// labels. It does not interpret expressions by itself.
///
/// # Table-level invariants
///
/// - Every modeling-label ID is owned by this table; labels for unknown
///   [`VariableID`] values are rejected by [`Self::new`] and [`Self::set_label`].
///
/// # Host-level invariants
///
/// This table does not validate facts that require the surrounding
/// [`crate::ParametricInstance`]. The host validates that parameter IDs are
/// disjoint from decision-variable IDs, that function bodies reference IDs from
/// the union of both tables, and that structural decision-variable positions
/// such as indicator / one-hot / SOS1 members never use parameter IDs.
///
/// # Table-local operations
///
/// The table supports construction from IDs and labels, read access, fresh
/// insertion of a parameter ID with its label, label updates for existing IDs,
/// and consuming the ID/label pair at serialization or conversion boundaries.
/// Duplicate insertion is rejected rather than interpreted as replacement.
///
/// Parameter-value substitution is not a table operation. It is the
/// [`crate::ParametricInstance::with_parameters`] root operation because it
/// rewrites every expression-bearing component into a concrete
/// [`crate::Instance`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParameterTable {
    ids: BTreeSet<VariableID>,
    labels: ParameterLabelStore,
}

impl ParameterTable {
    /// Construct a parameter table, rejecting labels for unknown IDs.
    pub fn new(ids: BTreeSet<VariableID>, labels: ParameterLabelStore) -> crate::Result<Self> {
        crate::modeling_label::validate_modeling_label_ids(&labels, &ids, "parameter")?;
        Ok(Self { ids, labels })
    }

    /// Construct a parameter table with no labels.
    pub fn from_ids(ids: BTreeSet<VariableID>) -> Self {
        Self {
            ids,
            labels: ParameterLabelStore::default(),
        }
    }

    /// Build from legacy v1 parameter rows, draining inline IDs and labels into
    /// the table-owned ID set and label store.
    pub fn from_v1_parameters<I>(parameters: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = crate::v1::Parameter>,
    {
        let mut ids = BTreeSet::new();
        let mut labels = ParameterLabelStore::default();
        for parameter in parameters {
            let id = VariableID::from(parameter.id);
            if !ids.insert(id) {
                crate::bail!(
                    { ?id },
                    "Duplicated parameter ID is found in definition: {id:?}",
                );
            }
            labels.insert(
                id,
                ParameterLabel {
                    name: parameter.name,
                    subscripts: parameter.subscripts,
                    parameters: parameter.parameters.into_iter().collect(),
                    description: parameter.description,
                },
            );
        }
        Self::new(ids, labels)
    }

    /// Parameter ID universe owned by this table.
    pub fn ids(&self) -> &BTreeSet<VariableID> {
        &self.ids
    }

    /// Per-parameter modeling label store.
    pub fn labels(&self) -> &ParameterLabelStore {
        &self.labels
    }

    /// Replace the modeling label for an existing parameter.
    pub fn set_label(&mut self, id: VariableID, label: ParameterLabel) -> crate::Result<()> {
        if !self.ids.contains(&id) {
            crate::bail!({ ?id }, "Modeling label references unknown parameter ID {id:?}");
        }
        self.labels.insert(id, label);
        Ok(())
    }

    /// Insert one fresh parameter ID and its modeling label.
    pub fn insert(&mut self, id: VariableID, label: ParameterLabel) -> crate::Result<()> {
        if !self.ids.insert(id) {
            crate::bail!({ ?id }, "Duplicate parameter ID: {id:?}");
        }
        self.labels.insert(id, label);
        Ok(())
    }

    pub fn contains_key(&self, id: &VariableID) -> bool {
        self.ids.contains(id)
    }

    pub fn keys(&self) -> std::collections::btree_set::Iter<'_, VariableID> {
        self.ids.iter()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Materialize one legacy v1 parameter row for API/protobuf boundaries.
    pub fn to_v1_parameter(&self, id: VariableID) -> Option<crate::v1::Parameter> {
        self.ids.contains(&id).then(|| {
            let label = self.labels.collect_for(id);
            crate::v1::Parameter {
                id: id.into_inner(),
                name: label.name,
                subscripts: label.subscripts,
                parameters: label.parameters.into_iter().collect::<HashMap<_, _>>(),
                description: label.description,
            }
        })
    }

    /// Materialize all legacy v1 parameter rows in ID order.
    pub fn to_v1_parameters(&self) -> Vec<crate::v1::Parameter> {
        self.ids
            .iter()
            .filter_map(|id| self.to_v1_parameter(*id))
            .collect()
    }

    /// Consume this table and materialize all legacy v1 parameter rows.
    pub fn into_v1_parameters(self) -> Vec<crate::v1::Parameter> {
        let ParameterTable { ids, mut labels } = self;
        ids.into_iter()
            .map(|id| {
                let label = labels.remove(id);
                crate::v1::Parameter {
                    id: id.into_inner(),
                    name: label.name,
                    subscripts: label.subscripts,
                    parameters: label.parameters.into_iter().collect::<HashMap<_, _>>(),
                    description: label.description,
                }
            })
            .collect()
    }
}

impl From<ParameterTable> for crate::v2::ParameterTable {
    fn from(table: ParameterTable) -> Self {
        Self {
            ids: table.ids.into_iter().map(|id| id.into_inner()).collect(),
            labels: crate::v2_io::modeling_label_store_to_v2_map(&table.labels),
        }
    }
}

impl Parse for crate::v2::ParameterTable {
    type Output = ParameterTable;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.ParameterTable";
        let mut ids = BTreeSet::new();
        for id in self.ids {
            let id = VariableID::from(id);
            if !ids.insert(id) {
                return Err(RawParseError::InvalidInstance(format!(
                    "Duplicated parameter ID is found in ommx.v2.ParameterTable: {id:?}",
                ))
                .context(message, "ids"));
            }
        }
        let labels = crate::v2_io::modeling_label_store_from_v2_map(self.labels);
        ParameterTable::new(ids, labels)
            .map_err(|e| RawParseError::InvalidInstance(e.to_string()).context(message, "ids"))
    }
}

impl LogicalMemoryProfile for ParameterTable {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.ids
            .visit_logical_memory(path.with("ids").as_mut(), visitor);
        self.labels
            .visit_logical_memory(path.with("labels").as_mut(), visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_orphan_labels() {
        let id = VariableID::from(1);
        let mut labels = ParameterLabelStore::default();
        labels.set_name(id, "p");

        let err = ParameterTable::new(BTreeSet::new(), labels).unwrap_err();
        assert!(
            err.to_string().contains("unknown parameter ID")
                && err.to_string().contains("VariableID(1)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn drains_legacy_v1_parameters_into_ids_and_labels() {
        let table = ParameterTable::from_v1_parameters([crate::v1::Parameter {
            id: 100,
            name: Some("p".to_string()),
            subscripts: vec![1, 2],
            parameters: HashMap::from([("scenario".to_string(), "base".to_string())]),
            description: Some("penalty".to_string()),
        }])
        .unwrap();

        let id = VariableID::from(100);
        assert!(table.contains_key(&id));
        assert_eq!(table.labels().name(id), Some("p"));
        assert_eq!(table.labels().subscripts(id), &[1, 2]);
        assert_eq!(
            table
                .labels()
                .parameters(id)
                .get("scenario")
                .map(String::as_str),
            Some("base")
        );
        assert_eq!(table.labels().description(id), Some("penalty"));
    }

    #[test]
    fn duplicate_insert_is_rejected_without_replacing_label() {
        let id = VariableID::from(100);
        let mut table = ParameterTable::default();

        table
            .insert(
                id,
                ParameterLabel {
                    name: Some("p".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        let err = table
            .insert(
                id,
                ParameterLabel {
                    name: Some("q".to_string()),
                    ..Default::default()
                },
            )
            .unwrap_err();

        assert!(err.to_string().contains("Duplicate parameter ID"));
        assert_eq!(table.labels().name(id), Some("p"));
    }

    #[test]
    fn rejects_duplicate_legacy_v1_parameter_ids() {
        let err = ParameterTable::from_v1_parameters([
            crate::v1::Parameter {
                id: 100,
                ..Default::default()
            },
            crate::v1::Parameter {
                id: 100,
                ..Default::default()
            },
        ])
        .unwrap_err();

        assert!(
            err.to_string().contains("Duplicated parameter ID")
                && err.to_string().contains("VariableID(100)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_duplicate_v2_parameter_ids() {
        let err = crate::v2::ParameterTable {
            ids: vec![100, 100],
            labels: Default::default(),
        }
        .parse(&())
        .unwrap_err();

        assert!(
            err.to_string().contains("Duplicated parameter ID")
                && err.to_string().contains("VariableID(100)"),
            "unexpected error: {err}"
        );
    }
}
