use std::collections::HashMap;

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

fn merge_solution_metadata_annotations(
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

fn overlay_solution_metadata_annotations(
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

fn insert_sample_set_metadata_annotations(
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

fn merge_sample_set_metadata_annotations(
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

fn overlay_sample_set_metadata_annotations(
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

pub fn instance_annotations(instance: &crate::v1::Instance) -> HashMap<String, String> {
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
    annotations
}

pub fn merge_instance_annotations(
    instance: &mut crate::v1::Instance,
    annotations: &HashMap<String, String>,
) {
    merge_description_annotations(
        &mut instance.description,
        annotations,
        "org.ommx.v1.instance",
    );
    merge_extension_annotations(&mut instance.annotations, annotations);
}

pub fn overlay_instance_annotations(
    instance: &mut crate::v1::Instance,
    annotations: &HashMap<String, String>,
) {
    overlay_description_annotations(
        &mut instance.description,
        annotations,
        "org.ommx.v1.instance",
    );
    overlay_extension_annotations(&mut instance.annotations, annotations);
}

pub fn parametric_instance_annotations(
    instance: &crate::v1::ParametricInstance,
) -> HashMap<String, String> {
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
    annotations
}

pub fn merge_parametric_instance_annotations(
    instance: &mut crate::v1::ParametricInstance,
    annotations: &HashMap<String, String>,
) {
    merge_description_annotations(
        &mut instance.description,
        annotations,
        "org.ommx.v1.parametric-instance",
    );
    merge_extension_annotations(&mut instance.annotations, annotations);
}

pub fn overlay_parametric_instance_annotations(
    instance: &mut crate::v1::ParametricInstance,
    annotations: &HashMap<String, String>,
) {
    overlay_description_annotations(
        &mut instance.description,
        annotations,
        "org.ommx.v1.parametric-instance",
    );
    overlay_extension_annotations(&mut instance.annotations, annotations);
}

pub fn solution_annotations(solution: &crate::v1::Solution) -> HashMap<String, String> {
    let mut annotations = HashMap::new();
    copy_extension_annotations(&mut annotations, &solution.annotations);
    insert_solution_metadata_annotations(
        &mut annotations,
        "org.ommx.v1.solution",
        &solution.metadata,
    );
    annotations
}

pub fn merge_solution_annotations(
    solution: &mut crate::v1::Solution,
    annotations: &HashMap<String, String>,
) {
    merge_solution_metadata_annotations(
        &mut solution.metadata,
        annotations,
        "org.ommx.v1.solution",
    );
    merge_extension_annotations(&mut solution.annotations, annotations);
}

pub fn overlay_solution_annotations(
    solution: &mut crate::v1::Solution,
    annotations: &HashMap<String, String>,
) {
    overlay_solution_metadata_annotations(
        &mut solution.metadata,
        annotations,
        "org.ommx.v1.solution",
    );
    overlay_extension_annotations(&mut solution.annotations, annotations);
}

pub fn sample_set_annotations(sample_set: &crate::v1::SampleSet) -> HashMap<String, String> {
    let mut annotations = HashMap::new();
    copy_extension_annotations(&mut annotations, &sample_set.annotations);
    insert_sample_set_metadata_annotations(
        &mut annotations,
        "org.ommx.v1.sample-set",
        &sample_set.metadata,
    );
    annotations
}

pub fn merge_sample_set_annotations(
    sample_set: &mut crate::v1::SampleSet,
    annotations: &HashMap<String, String>,
) {
    merge_sample_set_metadata_annotations(
        &mut sample_set.metadata,
        annotations,
        "org.ommx.v1.sample-set",
    );
    merge_extension_annotations(&mut sample_set.annotations, annotations);
}

pub fn overlay_sample_set_annotations(
    sample_set: &mut crate::v1::SampleSet,
    annotations: &HashMap<String, String>,
) {
    overlay_sample_set_metadata_annotations(
        &mut sample_set.metadata,
        annotations,
        "org.ommx.v1.sample-set",
    );
    overlay_extension_annotations(&mut sample_set.annotations, annotations);
}
