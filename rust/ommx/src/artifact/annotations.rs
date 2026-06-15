use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use derive_more::{Deref, DerefMut, From, Into};
use oci_spec::image::{Descriptor, Digest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::Message;

const RESERVED_PREFIX: &str = "org.ommx.v1.";

fn is_extension_annotation(key: &str) -> bool {
    !key.starts_with(RESERVED_PREFIX)
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

fn overlay_extension_annotations(
    target: &mut HashMap<String, String>,
    source: &HashMap<String, String>,
) {
    for (key, value) in source {
        if is_extension_annotation(key) {
            target.insert(key.clone(), value.clone());
        }
    }
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
                target.authors = authors.split(',').map(str::to_string).collect();
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

fn overlay_description_annotations(
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

fn insert_solution_metadata_annotations(
    annotations: &mut HashMap<String, String>,
    namespace: &str,
    metadata: &Option<crate::v1::solution::Metadata>,
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

fn merge_solution_metadata_annotations(
    target: &mut Option<crate::v1::solution::Metadata>,
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
        let target = target.get_or_insert_with(crate::v1::solution::Metadata::default);
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

fn overlay_solution_metadata_annotations(
    target: &mut Option<crate::v1::solution::Metadata>,
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
        let target = target.get_or_insert_with(crate::v1::solution::Metadata::default);
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

fn insert_sample_set_metadata_annotations(
    annotations: &mut HashMap<String, String>,
    namespace: &str,
    metadata: &Option<crate::v1::sample_set::Metadata>,
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

fn merge_sample_set_metadata_annotations(
    target: &mut Option<crate::v1::sample_set::Metadata>,
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
        let target = target.get_or_insert_with(crate::v1::sample_set::Metadata::default);
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

fn overlay_sample_set_metadata_annotations(
    target: &mut Option<crate::v1::sample_set::Metadata>,
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
        let target = target.get_or_insert_with(crate::v1::sample_set::Metadata::default);
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

pub(crate) fn encode_instance_layer(
    instance: &crate::Instance,
    annotations: InstanceAnnotations,
) -> (Vec<u8>, HashMap<String, String>) {
    let mut proto: crate::v1::Instance = instance.clone().into();
    annotations.merge_into_v1_instance(&mut proto);
    let descriptor_annotations = InstanceAnnotations::from_v1_instance(&proto).into_inner();
    (proto.encode_to_vec(), descriptor_annotations)
}

pub(crate) fn encode_parametric_instance_layer(
    instance: &crate::ParametricInstance,
    annotations: ParametricInstanceAnnotations,
) -> (Vec<u8>, HashMap<String, String>) {
    let mut proto: crate::v1::ParametricInstance = instance.clone().into();
    annotations.merge_into_v1_parametric_instance(&mut proto);
    let descriptor_annotations =
        ParametricInstanceAnnotations::from_v1_parametric_instance(&proto).into_inner();
    (proto.encode_to_vec(), descriptor_annotations)
}

pub(crate) fn encode_solution_layer(
    solution: &crate::Solution,
    annotations: SolutionAnnotations,
) -> (Vec<u8>, HashMap<String, String>) {
    let mut proto: crate::v1::Solution = solution.clone().into();
    annotations.merge_into_v1_solution(&mut proto);
    let descriptor_annotations = SolutionAnnotations::from_v1_solution(&proto).into_inner();
    (proto.encode_to_vec(), descriptor_annotations)
}

pub(crate) fn encode_sample_set_layer(
    sample_set: &crate::SampleSet,
    annotations: SampleSetAnnotations,
) -> (Vec<u8>, HashMap<String, String>) {
    let mut proto: crate::v1::SampleSet = sample_set.clone().into();
    annotations.merge_into_v1_sample_set(&mut proto);
    let descriptor_annotations = SampleSetAnnotations::from_v1_sample_set(&proto).into_inner();
    (proto.encode_to_vec(), descriptor_annotations)
}

/// Annotations for [`application/org.ommx.v1.instance`][crate::artifact::media_types::v1_instance]
#[derive(Debug, Default, Clone, PartialEq, From, Deref, DerefMut, Into, Serialize, Deserialize)]
pub struct InstanceAnnotations(HashMap<String, String>);

impl InstanceAnnotations {
    pub fn into_inner(self) -> HashMap<String, String> {
        self.0
    }

    fn get(&self, key: &str) -> Result<&String> {
        self.0.get(key).context(format!(
            "Annotation does not have the entry with the key `{key}`"
        ))
    }

    pub fn from_descriptor(desc: &Descriptor) -> Self {
        Self(desc.annotations().as_ref().cloned().unwrap_or_default())
    }

    pub fn from_v1_instance(instance: &crate::v1::Instance) -> Self {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &instance.annotations);
        insert_description_annotations(
            &mut annotations,
            "org.ommx.v1.instance",
            &instance.description,
        );
        annotations.insert(
            "org.ommx.v1.instance.variables".to_string(),
            instance.decision_variables.len().to_string(),
        );
        annotations.insert(
            "org.ommx.v1.instance.constraints".to_string(),
            instance.constraints.len().to_string(),
        );
        Self(annotations)
    }

    pub fn merge_into_v1_instance(&self, instance: &mut crate::v1::Instance) {
        merge_description_annotations(&mut instance.description, &self.0, "org.ommx.v1.instance");
        merge_extension_annotations(&mut instance.annotations, &self.0);
    }

    pub fn overlay_into_v1_instance(&self, instance: &mut crate::v1::Instance) {
        overlay_description_annotations(&mut instance.description, &self.0, "org.ommx.v1.instance");
        overlay_extension_annotations(&mut instance.annotations, &self.0);
    }

    pub fn set_title(&mut self, title: String) {
        self.0
            .insert("org.ommx.v1.instance.title".to_string(), title);
    }

    pub fn title(&self) -> Result<&String> {
        self.get("org.ommx.v1.instance.title")
    }

    pub fn set_created(&mut self, created: DateTime<Local>) {
        self.0.insert(
            "org.ommx.v1.instance.created".to_string(),
            created.to_rfc3339(),
        );
    }

    pub fn set_created_now(&mut self) {
        self.set_created(Local::now());
    }

    pub fn created(&self) -> Result<DateTime<Local>> {
        let created = self.get("org.ommx.v1.instance.created")?;
        Ok(DateTime::parse_from_rfc3339(created)?.with_timezone(&Local))
    }

    pub fn set_authors(&mut self, authors: Vec<String>) {
        self.0.insert(
            "org.ommx.v1.instance.authors".to_string(),
            authors.join(","),
        );
    }

    pub fn authors(&self) -> Result<impl Iterator<Item = &str>> {
        let authors = self.get("org.ommx.v1.instance.authors")?;
        Ok(authors.split(','))
    }

    pub fn set_license(&mut self, license: String) {
        self.0
            .insert("org.ommx.v1.instance.license".to_string(), license);
    }

    pub fn license(&self) -> Result<&String> {
        self.get("org.ommx.v1.instance.license")
    }

    pub fn set_dataset(&mut self, dataset: String) {
        self.0
            .insert("org.ommx.v1.instance.dataset".to_string(), dataset);
    }

    pub fn dataset(&self) -> Result<&String> {
        self.get("org.ommx.v1.instance.dataset")
    }

    pub fn set_variables(&mut self, variables: usize) {
        self.0.insert(
            "org.ommx.v1.instance.variables".to_string(),
            variables.to_string(),
        );
    }

    pub fn variables(&self) -> Result<usize> {
        let variables = self.get("org.ommx.v1.instance.variables")?;
        Ok(variables.parse()?)
    }

    pub fn set_constraints(&mut self, constraints: usize) {
        self.0.insert(
            "org.ommx.v1.instance.constraints".to_string(),
            constraints.to_string(),
        );
    }

    pub fn constraints(&self) -> Result<usize> {
        let constraints = self.get("org.ommx.v1.instance.constraints")?;
        Ok(constraints.parse()?)
    }

    /// Set other annotations. The key may not start with `org.ommx.v1.`, but must a valid reverse domain name.
    pub fn set_other(&mut self, key: String, value: String) {
        // TODO check key
        self.0.insert(key, value);
    }
}

/// Annotations for [`application/org.ommx.v1.parametric-instance`][crate::artifact::media_types::v1_parametric_instance]
#[derive(Debug, Default, Clone, PartialEq, From, Deref, DerefMut, Into, Serialize, Deserialize)]
pub struct ParametricInstanceAnnotations(HashMap<String, String>);

impl ParametricInstanceAnnotations {
    pub fn into_inner(self) -> HashMap<String, String> {
        self.0
    }

    fn get(&self, key: &str) -> Result<&String> {
        self.0.get(key).context(format!(
            "Annotation does not have the entry with the key `{key}`"
        ))
    }

    pub fn from_descriptor(desc: &Descriptor) -> Self {
        Self(desc.annotations().as_ref().cloned().unwrap_or_default())
    }

    pub fn from_v1_parametric_instance(instance: &crate::v1::ParametricInstance) -> Self {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &instance.annotations);
        insert_description_annotations(
            &mut annotations,
            "org.ommx.v1.parametric-instance",
            &instance.description,
        );
        annotations.insert(
            "org.ommx.v1.parametric-instance.variables".to_string(),
            instance.decision_variables.len().to_string(),
        );
        annotations.insert(
            "org.ommx.v1.parametric-instance.constraints".to_string(),
            instance.constraints.len().to_string(),
        );
        Self(annotations)
    }

    pub fn merge_into_v1_parametric_instance(&self, instance: &mut crate::v1::ParametricInstance) {
        merge_description_annotations(
            &mut instance.description,
            &self.0,
            "org.ommx.v1.parametric-instance",
        );
        merge_extension_annotations(&mut instance.annotations, &self.0);
    }

    pub fn overlay_into_v1_parametric_instance(
        &self,
        instance: &mut crate::v1::ParametricInstance,
    ) {
        overlay_description_annotations(
            &mut instance.description,
            &self.0,
            "org.ommx.v1.parametric-instance",
        );
        overlay_extension_annotations(&mut instance.annotations, &self.0);
    }

    pub fn set_title(&mut self, title: String) {
        self.0
            .insert("org.ommx.v1.parametric-instance.title".to_string(), title);
    }

    pub fn title(&self) -> Result<&String> {
        self.get("org.ommx.v1.parametric-instance.title")
    }

    pub fn set_created(&mut self, created: DateTime<Local>) {
        self.0.insert(
            "org.ommx.v1.parametric-instance.created".to_string(),
            created.to_rfc3339(),
        );
    }

    pub fn set_created_now(&mut self) {
        self.set_created(Local::now());
    }

    pub fn created(&self) -> Result<DateTime<Local>> {
        let created = self.get("org.ommx.v1.parametric-instance.created")?;
        Ok(DateTime::parse_from_rfc3339(created)?.with_timezone(&Local))
    }

    pub fn set_authors(&mut self, authors: Vec<String>) {
        self.0.insert(
            "org.ommx.v1.parametric-instance.authors".to_string(),
            authors.join(","),
        );
    }

    pub fn authors(&self) -> Result<impl Iterator<Item = &str>> {
        let authors = self.get("org.ommx.v1.parametric-instance.authors")?;
        Ok(authors.split(','))
    }

    pub fn set_license(&mut self, license: String) {
        self.0.insert(
            "org.ommx.v1.parametric-instance.license".to_string(),
            license,
        );
    }

    pub fn license(&self) -> Result<&String> {
        self.get("org.ommx.v1.parametric-instance.license")
    }

    pub fn set_dataset(&mut self, dataset: String) {
        self.0.insert(
            "org.ommx.v1.parametric-instance.dataset".to_string(),
            dataset,
        );
    }

    pub fn dataset(&self) -> Result<&String> {
        self.get("org.ommx.v1.parametric-instance.dataset")
    }

    pub fn set_variables(&mut self, variables: usize) {
        self.0.insert(
            "org.ommx.v1.parametric-instance.variables".to_string(),
            variables.to_string(),
        );
    }

    pub fn variables(&self) -> Result<usize> {
        let variables = self.get("org.ommx.v1.parametric-instance.variables")?;
        Ok(variables.parse()?)
    }

    pub fn set_constraints(&mut self, constraints: usize) {
        self.0.insert(
            "org.ommx.v1.parametric-instance.constraints".to_string(),
            constraints.to_string(),
        );
    }

    pub fn constraints(&self) -> Result<usize> {
        let constraints = self.get("org.ommx.v1.parametric-instance.constraints")?;
        Ok(constraints.parse()?)
    }

    /// Set other annotations. The key may not start with `org.ommx.v1.`, but must a valid reverse domain name.
    pub fn set_other(&mut self, key: String, value: String) {
        // TODO check key
        self.0.insert(key, value);
    }
}

/// Annotations for [`application/org.ommx.v1.solution`][crate::artifact::media_types::v1_solution]
#[derive(Debug, Default, Clone, PartialEq, From, Deref, DerefMut, Into, Serialize, Deserialize)]
pub struct SolutionAnnotations(HashMap<String, String>);

impl SolutionAnnotations {
    pub fn into_inner(self) -> HashMap<String, String> {
        self.0
    }

    pub fn from_descriptor(desc: &Descriptor) -> Self {
        Self(desc.annotations().as_ref().cloned().unwrap_or_default())
    }

    pub fn from_v1_solution(solution: &crate::v1::Solution) -> Self {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &solution.annotations);
        insert_solution_metadata_annotations(
            &mut annotations,
            "org.ommx.v1.solution",
            &solution.metadata,
        );
        Self(annotations)
    }

    pub fn merge_into_v1_solution(&self, solution: &mut crate::v1::Solution) {
        merge_solution_metadata_annotations(
            &mut solution.metadata,
            &self.0,
            "org.ommx.v1.solution",
        );
        merge_extension_annotations(&mut solution.annotations, &self.0);
    }

    pub fn overlay_into_v1_solution(&self, solution: &mut crate::v1::Solution) {
        overlay_solution_metadata_annotations(
            &mut solution.metadata,
            &self.0,
            "org.ommx.v1.solution",
        );
        overlay_extension_annotations(&mut solution.annotations, &self.0);
    }

    pub fn set_start(&mut self, start: DateTime<Local>) {
        self.0
            .insert("org.ommx.v1.solution.start".to_string(), start.to_rfc3339());
    }

    pub fn start(&self) -> Result<DateTime<Local>> {
        let start = self.0.get("org.ommx.v1.solution.start").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.start`",
        )?;
        Ok(DateTime::parse_from_rfc3339(start)?.with_timezone(&Local))
    }

    pub fn set_end(&mut self, end: DateTime<Local>) {
        self.0
            .insert("org.ommx.v1.solution.end".to_string(), end.to_rfc3339());
    }

    pub fn end(&self) -> Result<DateTime<Local>> {
        let end = self.0.get("org.ommx.v1.solution.end").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.end`",
        )?;
        Ok(DateTime::parse_from_rfc3339(end)?.with_timezone(&Local))
    }

    /// Set `org.ommx.v1.solution.instance`
    pub fn set_instance(&mut self, digest: Digest) {
        self.0.insert(
            "org.ommx.v1.solution.instance".to_string(),
            digest.to_string(),
        );
    }

    /// Get `org.ommx.v1.solution.instance`
    pub fn instance(&self) -> Result<Digest> {
        let digest = self.0.get("org.ommx.v1.solution.instance").context(
            "Annotation does not have the entry with the key `org.ommx.v1.solution.instance`",
        )?;
        digest.parse().context("Failed to parse digest")
    }

    /// Set `org.ommx.v1.solution.solver`
    pub fn set_solver(&mut self, solver: impl Serialize) -> Result<()> {
        self.0.insert(
            "org.ommx.v1.solution.solver".to_string(),
            serde_json::to_string(&solver)?,
        );
        Ok(())
    }

    /// Get `org.ommx.v1.solution.solver`
    pub fn solver<'s: 'de, 'de, S: Deserialize<'de>>(&'s self) -> Result<S> {
        Ok(serde_json::from_str(
            self.0.get("org.ommx.v1.solution.solver").context(
                "Annotation does not have the entry with the key `org.ommx.v1.solution.solver`",
            )?,
        )?)
    }

    /// Set `org.ommx.v1.solution.parameters`
    pub fn set_parameters(&mut self, parameters: impl Serialize) -> Result<()> {
        self.0.insert(
            "org.ommx.v1.solution.parameters".to_string(),
            serde_json::to_string(&parameters)?,
        );
        Ok(())
    }

    /// Get `org.ommx.v1.solution.parameters`
    pub fn parameters<'s: 'de, 'de, P: Deserialize<'de>>(&'s self) -> Result<P> {
        Ok(serde_json::from_str(
            self.0.get("org.ommx.v1.solution.parameters").context(
                "Annotation does not have the entry with the key `org.ommx.v1.solution.parameters`",
            )?,
        )?)
    }

    /// Set other annotations
    pub fn set_other(&mut self, key: String, value: String) {
        // TODO check key
        self.0.insert(key, value);
    }
}

#[derive(Debug, Default, Clone, PartialEq, From, Deref, DerefMut, Into, Serialize, Deserialize)]
pub struct SampleSetAnnotations(HashMap<String, String>);

impl SampleSetAnnotations {
    pub fn into_inner(self) -> HashMap<String, String> {
        self.0
    }

    pub fn from_descriptor(desc: &Descriptor) -> Self {
        Self(desc.annotations().as_ref().cloned().unwrap_or_default())
    }

    pub fn from_v1_sample_set(sample_set: &crate::v1::SampleSet) -> Self {
        let mut annotations = HashMap::new();
        copy_extension_annotations(&mut annotations, &sample_set.annotations);
        insert_sample_set_metadata_annotations(
            &mut annotations,
            "org.ommx.v1.sample-set",
            &sample_set.metadata,
        );
        Self(annotations)
    }

    pub fn merge_into_v1_sample_set(&self, sample_set: &mut crate::v1::SampleSet) {
        merge_sample_set_metadata_annotations(
            &mut sample_set.metadata,
            &self.0,
            "org.ommx.v1.sample-set",
        );
        merge_extension_annotations(&mut sample_set.annotations, &self.0);
    }

    pub fn overlay_into_v1_sample_set(&self, sample_set: &mut crate::v1::SampleSet) {
        overlay_sample_set_metadata_annotations(
            &mut sample_set.metadata,
            &self.0,
            "org.ommx.v1.sample-set",
        );
        overlay_extension_annotations(&mut sample_set.annotations, &self.0);
    }

    pub fn set_start(&mut self, start: DateTime<Local>) {
        self.0.insert(
            "org.ommx.v1.sample-set.start".to_string(),
            start.to_rfc3339(),
        );
    }

    pub fn start(&self) -> Result<DateTime<Local>> {
        let start = self.0.get("org.ommx.v1.sample-set.start").context(
            "Annotation does not have the entry with the key `org.ommx.v1.sample-set.start`",
        )?;
        Ok(DateTime::parse_from_rfc3339(start)?.with_timezone(&Local))
    }

    pub fn set_end(&mut self, end: DateTime<Local>) {
        self.0
            .insert("org.ommx.v1.sample-set.end".to_string(), end.to_rfc3339());
    }

    pub fn end(&self) -> Result<DateTime<Local>> {
        let end = self.0.get("org.ommx.v1.sample-set.end").context(
            "Annotation does not have the entry with the key `org.ommx.v1.sample-set.end`",
        )?;
        Ok(DateTime::parse_from_rfc3339(end)?.with_timezone(&Local))
    }

    /// Set `org.ommx.v1.sample-set.instance`
    pub fn set_instance(&mut self, digest: Digest) {
        self.0.insert(
            "org.ommx.v1.sample-set.instance".to_string(),
            digest.to_string(),
        );
    }

    /// Get `org.ommx.v1.sample-set.instance`
    pub fn instance(&self) -> Result<Digest> {
        let digest = self.0.get("org.ommx.v1.sample-set.instance").context(
            "Annotation does not have the entry with the key `org.ommx.v1.sample-set.instance`",
        )?;
        digest.parse().context("Failed to parse digest")
    }

    /// Set `org.ommx.v1.sample-set.solver`
    pub fn set_solver(&mut self, solver: impl Serialize) -> Result<()> {
        self.0.insert(
            "org.ommx.v1.sample-set.solver".to_string(),
            serde_json::to_string(&solver)?,
        );
        Ok(())
    }

    /// Get `org.ommx.v1.sample-set.solver`
    pub fn solver<'s: 'de, 'de, S: Deserialize<'de>>(&'s self) -> Result<S> {
        Ok(serde_json::from_str(
            self.0.get("org.ommx.v1.sample-set.solver").context(
                "Annotation does not have the entry with the key `org.ommx.v1.sample-set.solver`",
            )?,
        )?)
    }

    /// Set `org.ommx.v1.sample-set.parameters`
    pub fn set_parameters(&mut self, parameters: impl Serialize) -> Result<()> {
        self.0.insert(
            "org.ommx.v1.sample-set.parameters".to_string(),
            serde_json::to_string(&parameters)?,
        );
        Ok(())
    }

    /// Get `org.ommx.v1.sample-set.parameters`
    pub fn parameters<'s: 'de, 'de, P: Deserialize<'de>>(&'s self) -> Result<P> {
        Ok(serde_json::from_str(
            self.0.get("org.ommx.v1.sample-set.parameters").context(
                "Annotation does not have the entry with the key `org.ommx.v1.sample-set.parameters`",
            )?,
        )?)
    }

    pub fn set_other(&mut self, key: String, value: String) {
        // TODO check key
        self.0.insert(key, value);
    }
}
