use std::collections::HashMap;

const RESERVED_PREFIX: &str = "org.ommx.v1.";

/// Flat annotation keys defined by OMMX.
///
/// These constants describe the public flat annotation vocabulary mirrored to
/// OCI descriptors and Python `annotations` views. Domain objects remain the
/// source of truth for the corresponding metadata.
pub mod annotation_keys {
    /// Storage compression applied to an Experiment Attachment OCI layer.
    pub const ATTACHMENT_COMPRESSION: &str = "org.ommx.v1.attachment.compression";

    pub const INSTANCE_NAMESPACE: &str = "org.ommx.v1.instance";
    pub const INSTANCE_TITLE: &str = "org.ommx.v1.instance.title";
    pub const INSTANCE_AUTHORS: &str = "org.ommx.v1.instance.authors";
    pub const INSTANCE_LICENSE: &str = "org.ommx.v1.instance.license";
    pub const INSTANCE_DATASET: &str = "org.ommx.v1.instance.dataset";
    pub const INSTANCE_VARIABLES: &str = "org.ommx.v1.instance.variables";
    pub const INSTANCE_CONSTRAINTS: &str = "org.ommx.v1.instance.constraints";

    pub const PARAMETRIC_INSTANCE_NAMESPACE: &str = "org.ommx.v1.parametric-instance";
    pub const PARAMETRIC_INSTANCE_VARIABLES: &str = "org.ommx.v1.parametric-instance.variables";
    pub const PARAMETRIC_INSTANCE_CONSTRAINTS: &str = "org.ommx.v1.parametric-instance.constraints";

    pub const SOLUTION_NAMESPACE: &str = "org.ommx.v1.solution";
    pub const SAMPLE_SET_NAMESPACE: &str = "org.ommx.v1.sample-set";
}

/// Return true when an annotation key is reserved for OMMX-defined metadata.
pub fn is_reserved_annotation_key(key: &str) -> bool {
    key.starts_with(RESERVED_PREFIX)
}

fn is_extension_annotation(key: &str) -> bool {
    !is_reserved_annotation_key(key)
}

fn copy_extension_annotations(
    target: &mut HashMap<String, String>,
    source: &HashMap<String, String>,
) {
    for (key, value) in source {
        if is_extension_annotation(key) {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn merge_extension_annotations(
    target: &mut HashMap<String, String>,
    source: &HashMap<String, String>,
) {
    for (key, value) in source {
        if is_extension_annotation(key) {
            target.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
}

fn replace_extension_annotations(
    target: &mut HashMap<String, String>,
    source: &HashMap<String, String>,
) {
    target.clear();
    copy_extension_annotations(target, source);
}

/// Crate-internal protobuf serializers use this to keep v1 extension maps free
/// of OMMX-reserved metadata keys even if a domain object's raw annotation map
/// was mutated directly.
pub(crate) fn protobuf_extension_annotations(
    annotations: HashMap<String, String>,
) -> HashMap<String, String> {
    annotations
        .into_iter()
        .filter(|(key, _)| is_extension_annotation(key))
        .collect()
}

fn description_mut(
    description: &mut Option<crate::v1::instance::Description>,
) -> &mut crate::v1::instance::Description {
    description.get_or_insert_with(crate::v1::instance::Description::default)
}

fn parse_authors_annotation(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|author| !author.is_empty())
        .map(str::to_string)
        .collect()
}

fn insert_description_annotations(
    annotations: &mut HashMap<String, String>,
    namespace: &str,
    description: &Option<crate::v1::instance::Description>,
) {
    let Some(description) = description else {
        return;
    };
    if let Some(value) = &description.name {
        annotations.insert(format!("{namespace}.title"), value.clone());
    }
    if let Some(value) = &description.description {
        annotations.insert(format!("{namespace}.description"), value.clone());
    }
    if !description.authors.is_empty() {
        annotations.insert(
            format!("{namespace}.authors"),
            description.authors.join(","),
        );
    }
    if let Some(value) = &description.created_by {
        annotations.insert(format!("{namespace}.created_by"), value.clone());
    }
    if let Some(value) = &description.created {
        annotations.insert(format!("{namespace}.created"), value.clone());
    }
    if let Some(value) = &description.license {
        annotations.insert(format!("{namespace}.license"), value.clone());
    }
    if let Some(value) = &description.dataset {
        annotations.insert(format!("{namespace}.dataset"), value.clone());
    }
}

fn merge_description_annotations(
    target: &mut Option<crate::v1::instance::Description>,
    source: &HashMap<String, String>,
    namespace: &str,
) {
    let title_key = format!("{namespace}.title");
    let description_key = format!("{namespace}.description");
    let authors_key = format!("{namespace}.authors");
    let created_by_key = format!("{namespace}.created_by");
    let created_key = format!("{namespace}.created");
    let license_key = format!("{namespace}.license");
    let dataset_key = format!("{namespace}.dataset");

    if source.contains_key(&title_key)
        || source.contains_key(&description_key)
        || source.contains_key(&authors_key)
        || source.contains_key(&created_by_key)
        || source.contains_key(&created_key)
        || source.contains_key(&license_key)
        || source.contains_key(&dataset_key)
    {
        let target = description_mut(target);
        if target.name.is_none() {
            target.name = source.get(&title_key).cloned();
        }
        if target.description.is_none() {
            target.description = source.get(&description_key).cloned();
        }
        if target.authors.is_empty() {
            if let Some(authors) = source.get(&authors_key).filter(|v| !v.is_empty()) {
                target.authors = parse_authors_annotation(authors);
            }
        }
        if target.created_by.is_none() {
            target.created_by = source.get(&created_by_key).cloned();
        }
        if target.created.is_none() {
            target.created = source.get(&created_key).cloned();
        }
        if target.license.is_none() {
            target.license = source.get(&license_key).cloned();
        }
        if target.dataset.is_none() {
            target.dataset = source.get(&dataset_key).cloned();
        }
    }
}

fn replace_description_annotations(
    target: &mut Option<crate::v1::instance::Description>,
    source: &HashMap<String, String>,
    namespace: &str,
) {
    let title_key = format!("{namespace}.title");
    let description_key = format!("{namespace}.description");
    let authors_key = format!("{namespace}.authors");
    let created_by_key = format!("{namespace}.created_by");
    let created_key = format!("{namespace}.created");
    let license_key = format!("{namespace}.license");
    let dataset_key = format!("{namespace}.dataset");

    if source.contains_key(&title_key)
        || source.contains_key(&description_key)
        || source.contains_key(&authors_key)
        || source.contains_key(&created_by_key)
        || source.contains_key(&created_key)
        || source.contains_key(&license_key)
        || source.contains_key(&dataset_key)
    {
        let target = description_mut(target);
        if let Some(value) = source.get(&title_key) {
            target.name = Some(value.clone());
        }
        if let Some(value) = source.get(&description_key) {
            target.description = Some(value.clone());
        }
        if let Some(value) = source.get(&authors_key) {
            target.authors = parse_authors_annotation(value);
        }
        if let Some(value) = source.get(&created_by_key) {
            target.created_by = Some(value.clone());
        }
        if let Some(value) = source.get(&created_key) {
            target.created = Some(value.clone());
        }
        if let Some(value) = source.get(&license_key) {
            target.license = Some(value.clone());
        }
        if let Some(value) = source.get(&dataset_key) {
            target.dataset = Some(value.clone());
        }
    }
}

fn insert_process_metadata_annotations(
    annotations: &mut HashMap<String, String>,
    namespace: &str,
    metadata: &Option<crate::v1::ProcessMetadata>,
) {
    let Some(metadata) = metadata else {
        return;
    };
    if let Some(value) = &metadata.instance {
        annotations.insert(format!("{namespace}.instance"), value.clone());
    }
    if let Some(value) = &metadata.solver {
        annotations.insert(format!("{namespace}.solver"), value.clone());
    }
    if let Some(value) = &metadata.parameters {
        annotations.insert(format!("{namespace}.parameters"), value.clone());
    }
    if let Some(value) = &metadata.start {
        annotations.insert(format!("{namespace}.start"), value.clone());
    }
    if let Some(value) = &metadata.end {
        annotations.insert(format!("{namespace}.end"), value.clone());
    }
}

fn merge_process_metadata_annotations(
    target: &mut Option<crate::v1::ProcessMetadata>,
    source: &HashMap<String, String>,
    namespace: &str,
) {
    let instance_key = format!("{namespace}.instance");
    let solver_key = format!("{namespace}.solver");
    let parameters_key = format!("{namespace}.parameters");
    let start_key = format!("{namespace}.start");
    let end_key = format!("{namespace}.end");

    if source.contains_key(&instance_key)
        || source.contains_key(&solver_key)
        || source.contains_key(&parameters_key)
        || source.contains_key(&start_key)
        || source.contains_key(&end_key)
    {
        let target = target.get_or_insert_with(crate::v1::ProcessMetadata::default);
        if target.instance.is_none() {
            target.instance = source.get(&instance_key).cloned();
        }
        if target.solver.is_none() {
            target.solver = source.get(&solver_key).cloned();
        }
        if target.parameters.is_none() {
            target.parameters = source.get(&parameters_key).cloned();
        }
        if target.start.is_none() {
            target.start = source.get(&start_key).cloned();
        }
        if target.end.is_none() {
            target.end = source.get(&end_key).cloned();
        }
    }
}

fn replace_process_metadata_annotations(
    target: &mut Option<crate::v1::ProcessMetadata>,
    source: &HashMap<String, String>,
    namespace: &str,
) {
    let instance_key = format!("{namespace}.instance");
    let solver_key = format!("{namespace}.solver");
    let parameters_key = format!("{namespace}.parameters");
    let start_key = format!("{namespace}.start");
    let end_key = format!("{namespace}.end");

    if source.contains_key(&instance_key)
        || source.contains_key(&solver_key)
        || source.contains_key(&parameters_key)
        || source.contains_key(&start_key)
        || source.contains_key(&end_key)
    {
        let target = target.get_or_insert_with(crate::v1::ProcessMetadata::default);
        if let Some(value) = source.get(&instance_key) {
            target.instance = Some(value.clone());
        }
        if let Some(value) = source.get(&solver_key) {
            target.solver = Some(value.clone());
        }
        if let Some(value) = source.get(&parameters_key) {
            target.parameters = Some(value.clone());
        }
        if let Some(value) = source.get(&start_key) {
            target.start = Some(value.clone());
        }
        if let Some(value) = source.get(&end_key) {
            target.end = Some(value.clone());
        }
    }
}

/// Domain-level view of OMMX flat annotations.
///
/// OMMX domain objects are the source of truth. This trait exposes the flat
/// annotation map used by Python and Artifact descriptors without moving the
/// mapping rules into those serialization layers.
pub trait FlatAnnotations {
    fn flat_annotations(&self) -> HashMap<String, String>;
    fn merge_annotations(&mut self, annotations: &HashMap<String, String>);
    fn replace_annotations(&mut self, annotations: HashMap<String, String>);
}

impl FlatAnnotations for crate::Instance {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &self.annotations);
        insert_description_annotations(
            &mut annotations,
            annotation_keys::INSTANCE_NAMESPACE,
            &self.description,
        );
        annotations.insert(
            annotation_keys::INSTANCE_VARIABLES.to_string(),
            self.decision_variables().len().to_string(),
        );
        annotations.insert(
            annotation_keys::INSTANCE_CONSTRAINTS.to_string(),
            (self.constraints().len()
                + self.indicator_constraints().len()
                + self.one_hot_constraints().len()
                + self.sos1_constraints().len())
            .to_string(),
        );
        annotations
    }

    fn merge_annotations(&mut self, annotations: &HashMap<String, String>) {
        merge_description_annotations(
            &mut self.description,
            annotations,
            annotation_keys::INSTANCE_NAMESPACE,
        );
        merge_extension_annotations(&mut self.annotations, annotations);
    }

    fn replace_annotations(&mut self, annotations: HashMap<String, String>) {
        self.description = None;
        replace_description_annotations(
            &mut self.description,
            &annotations,
            annotation_keys::INSTANCE_NAMESPACE,
        );
        replace_extension_annotations(&mut self.annotations, &annotations);
    }
}

impl FlatAnnotations for crate::ParametricInstance {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &self.annotations);
        insert_description_annotations(
            &mut annotations,
            annotation_keys::PARAMETRIC_INSTANCE_NAMESPACE,
            &self.description,
        );
        annotations.insert(
            annotation_keys::PARAMETRIC_INSTANCE_VARIABLES.to_string(),
            self.decision_variables().len().to_string(),
        );
        annotations.insert(
            annotation_keys::PARAMETRIC_INSTANCE_CONSTRAINTS.to_string(),
            (self.constraints().len()
                + self.indicator_constraints().len()
                + self.one_hot_constraints().len()
                + self.sos1_constraints().len())
            .to_string(),
        );
        annotations
    }

    fn merge_annotations(&mut self, annotations: &HashMap<String, String>) {
        merge_description_annotations(
            &mut self.description,
            annotations,
            annotation_keys::PARAMETRIC_INSTANCE_NAMESPACE,
        );
        merge_extension_annotations(&mut self.annotations, annotations);
    }

    fn replace_annotations(&mut self, annotations: HashMap<String, String>) {
        self.description = None;
        replace_description_annotations(
            &mut self.description,
            &annotations,
            annotation_keys::PARAMETRIC_INSTANCE_NAMESPACE,
        );
        replace_extension_annotations(&mut self.annotations, &annotations);
    }
}

impl FlatAnnotations for crate::Solution {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &self.annotations);
        insert_process_metadata_annotations(
            &mut annotations,
            annotation_keys::SOLUTION_NAMESPACE,
            &self.metadata,
        );
        annotations
    }

    fn merge_annotations(&mut self, annotations: &HashMap<String, String>) {
        merge_process_metadata_annotations(
            &mut self.metadata,
            annotations,
            annotation_keys::SOLUTION_NAMESPACE,
        );
        merge_extension_annotations(&mut self.annotations, annotations);
    }

    fn replace_annotations(&mut self, annotations: HashMap<String, String>) {
        self.metadata = None;
        replace_process_metadata_annotations(
            &mut self.metadata,
            &annotations,
            annotation_keys::SOLUTION_NAMESPACE,
        );
        replace_extension_annotations(&mut self.annotations, &annotations);
    }
}

impl FlatAnnotations for crate::SampleSet {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &self.annotations);
        insert_process_metadata_annotations(
            &mut annotations,
            annotation_keys::SAMPLE_SET_NAMESPACE,
            &self.metadata,
        );
        annotations
    }

    fn merge_annotations(&mut self, annotations: &HashMap<String, String>) {
        merge_process_metadata_annotations(
            &mut self.metadata,
            annotations,
            annotation_keys::SAMPLE_SET_NAMESPACE,
        );
        merge_extension_annotations(&mut self.annotations, annotations);
    }

    fn replace_annotations(&mut self, annotations: HashMap<String, String>) {
        self.metadata = None;
        replace_process_metadata_annotations(
            &mut self.metadata,
            &annotations,
            annotation_keys::SAMPLE_SET_NAMESPACE,
        );
        replace_extension_annotations(&mut self.annotations, &annotations);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn instance_flat_annotations_project_description_and_extensions() {
        let mut instance = crate::Instance::default();
        instance.description = Some(crate::v1::instance::Description {
            name: Some("demo".to_string()),
            license: Some("MIT".to_string()),
            ..Default::default()
        });
        instance.annotations =
            HashMap::from([("org.example.owner".to_string(), "alice".to_string())]);

        let annotations = instance.flat_annotations();
        assert_eq!(
            annotations.get(annotation_keys::INSTANCE_TITLE),
            Some(&"demo".to_string())
        );
        assert_eq!(
            annotations.get(annotation_keys::INSTANCE_LICENSE),
            Some(&"MIT".to_string())
        );
        assert_eq!(
            annotations.get("org.example.owner"),
            Some(&"alice".to_string())
        );

        instance.replace_annotations(HashMap::from([
            (
                annotation_keys::INSTANCE_TITLE.to_string(),
                "updated".to_string(),
            ),
            (
                annotation_keys::INSTANCE_VARIABLES.to_string(),
                "999".to_string(),
            ),
            ("org.example.owner".to_string(), "bob".to_string()),
        ]));

        assert_eq!(
            instance
                .description
                .as_ref()
                .and_then(|desc| desc.name.as_deref()),
            Some("updated")
        );
        assert_eq!(
            instance.annotations,
            HashMap::from([("org.example.owner".to_string(), "bob".to_string())])
        );
        assert_eq!(
            instance
                .flat_annotations()
                .get(annotation_keys::INSTANCE_VARIABLES),
            Some(&"0".to_string())
        );
    }

    #[test]
    fn description_author_annotations_are_trimmed() {
        let mut instance = crate::Instance::default();
        instance.merge_annotations(&HashMap::from([(
            annotation_keys::INSTANCE_AUTHORS.to_string(),
            "Alice, Bob,, Carol , ".to_string(),
        )]));
        assert_eq!(
            instance
                .description
                .as_ref()
                .map(|desc| desc.authors.as_slice()),
            Some(&["Alice".to_string(), "Bob".to_string(), "Carol".to_string()][..])
        );

        instance.replace_annotations(HashMap::from([(
            annotation_keys::INSTANCE_AUTHORS.to_string(),
            " Dave, , Eve ".to_string(),
        )]));
        assert_eq!(
            instance
                .description
                .as_ref()
                .map(|desc| desc.authors.as_slice()),
            Some(&["Dave".to_string(), "Eve".to_string()][..])
        );
    }

    #[test]
    fn flat_annotations_count_active_special_constraints() {
        let decision_variables = BTreeMap::from([
            (
                crate::VariableID::from(1),
                crate::DecisionVariable::binary(),
            ),
            (
                crate::VariableID::from(2),
                crate::DecisionVariable::binary(),
            ),
        ]);
        let one_hot = crate::OneHotConstraint::new(BTreeSet::from([
            crate::VariableID::from(1),
            crate::VariableID::from(2),
        ]))
        .unwrap();
        let instance = crate::Instance::builder()
            .sense(crate::Sense::Minimize)
            .objective(crate::Function::Zero)
            .decision_variables(decision_variables)
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                crate::OneHotConstraintID::from(10),
                one_hot,
            )]))
            .build()
            .unwrap();

        let annotations = instance.flat_annotations();
        assert_eq!(
            annotations.get(annotation_keys::INSTANCE_CONSTRAINTS),
            Some(&"1".to_string())
        );

        let parametric: crate::ParametricInstance = instance.into();
        let annotations = parametric.flat_annotations();
        assert_eq!(
            annotations.get(annotation_keys::PARAMETRIC_INSTANCE_CONSTRAINTS),
            Some(&"1".to_string())
        );
    }
}
