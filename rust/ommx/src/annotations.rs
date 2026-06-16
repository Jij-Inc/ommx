use std::collections::HashMap;

const RESERVED_PREFIX: &str = "org.ommx.v1.";

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

fn replace_extension_annotations(
    target: &mut HashMap<String, String>,
    source: &HashMap<String, String>,
) {
    target.clear();
    copy_extension_annotations(target, source);
}

fn description_mut(
    description: &mut Option<crate::v1::instance::Description>,
) -> &mut crate::v1::instance::Description {
    description.get_or_insert_with(crate::v1::instance::Description::default)
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
            target.authors = if value.is_empty() {
                Vec::new()
            } else {
                value.split(',').map(str::to_string).collect()
            };
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
    fn replace_annotations(&mut self, annotations: HashMap<String, String>);

    fn insert_flat_annotation(&mut self, key: String, value: String) {
        let mut annotations = self.flat_annotations();
        annotations.insert(key, value);
        self.replace_annotations(annotations);
    }
}

impl FlatAnnotations for crate::Instance {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &self.annotations);
        insert_description_annotations(&mut annotations, "org.ommx.v1.instance", &self.description);
        annotations.insert(
            "org.ommx.v1.instance.variables".to_string(),
            self.decision_variables().len().to_string(),
        );
        annotations.insert(
            "org.ommx.v1.instance.constraints".to_string(),
            self.constraints().len().to_string(),
        );
        annotations
    }

    fn replace_annotations(&mut self, annotations: HashMap<String, String>) {
        self.description = None;
        replace_description_annotations(
            &mut self.description,
            &annotations,
            "org.ommx.v1.instance",
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
            "org.ommx.v1.parametric-instance",
            &self.description,
        );
        annotations.insert(
            "org.ommx.v1.parametric-instance.variables".to_string(),
            self.decision_variables().len().to_string(),
        );
        annotations.insert(
            "org.ommx.v1.parametric-instance.constraints".to_string(),
            self.constraints().len().to_string(),
        );
        annotations
    }

    fn replace_annotations(&mut self, annotations: HashMap<String, String>) {
        self.description = None;
        replace_description_annotations(
            &mut self.description,
            &annotations,
            "org.ommx.v1.parametric-instance",
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
            "org.ommx.v1.solution",
            &self.metadata,
        );
        annotations
    }

    fn replace_annotations(&mut self, annotations: HashMap<String, String>) {
        self.metadata = None;
        replace_process_metadata_annotations(
            &mut self.metadata,
            &annotations,
            "org.ommx.v1.solution",
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
            "org.ommx.v1.sample-set",
            &self.metadata,
        );
        annotations
    }

    fn replace_annotations(&mut self, annotations: HashMap<String, String>) {
        self.metadata = None;
        replace_process_metadata_annotations(
            &mut self.metadata,
            &annotations,
            "org.ommx.v1.sample-set",
        );
        replace_extension_annotations(&mut self.annotations, &annotations);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            annotations.get("org.ommx.v1.instance.title"),
            Some(&"demo".to_string())
        );
        assert_eq!(
            annotations.get("org.ommx.v1.instance.license"),
            Some(&"MIT".to_string())
        );
        assert_eq!(
            annotations.get("org.example.owner"),
            Some(&"alice".to_string())
        );

        instance.replace_annotations(HashMap::from([
            (
                "org.ommx.v1.instance.title".to_string(),
                "updated".to_string(),
            ),
            (
                "org.ommx.v1.instance.variables".to_string(),
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
                .get("org.ommx.v1.instance.variables"),
            Some(&"0".to_string())
        );
    }
}
