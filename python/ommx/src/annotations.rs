use anyhow::Result;
use ommx::Parse;
use pyo3::types::{PyAnyMethods, PyDictMethods};
use std::collections::HashMap;

/// Normalize annotation namespace to ensure it ends with "."
pub fn normalize_namespace(ns: &str) -> String {
    if ns.ends_with('.') {
        ns.to_string()
    } else {
        format!("{ns}.")
    }
}

pub fn instance_to_v1_with_annotations(instance: &crate::Instance) -> ommx::v1::Instance {
    instance.inner.clone().into()
}

pub fn instance_from_v1_with_descriptor_annotations(
    mut proto: ommx::v1::Instance,
    descriptor_annotations: HashMap<String, String>,
) -> Result<crate::Instance> {
    ommx::artifact::merge_instance_annotations(&mut proto, &descriptor_annotations);
    Ok(crate::Instance {
        inner: proto.try_into()?,
    })
}

pub fn parametric_instance_to_v1_with_annotations(
    instance: &crate::ParametricInstance,
) -> ommx::v1::ParametricInstance {
    instance.inner.clone().into()
}

pub fn parametric_instance_from_v1_with_descriptor_annotations(
    mut proto: ommx::v1::ParametricInstance,
    descriptor_annotations: HashMap<String, String>,
) -> Result<crate::ParametricInstance> {
    ommx::artifact::merge_parametric_instance_annotations(&mut proto, &descriptor_annotations);
    Ok(crate::ParametricInstance {
        inner: proto.parse(&())?,
    })
}

pub fn solution_to_v1_with_annotations(solution: &crate::Solution) -> ommx::v1::Solution {
    solution.inner.clone().into()
}

pub fn solution_from_v1_with_descriptor_annotations(
    mut proto: ommx::v1::Solution,
    descriptor_annotations: HashMap<String, String>,
) -> Result<crate::Solution> {
    ommx::artifact::merge_solution_annotations(&mut proto, &descriptor_annotations);
    Ok(crate::Solution {
        inner: proto.parse(&())?,
    })
}

pub fn sample_set_to_v1_with_annotations(sample_set: &crate::SampleSet) -> ommx::v1::SampleSet {
    sample_set.inner.clone().into()
}

pub fn sample_set_from_v1_with_descriptor_annotations(
    mut proto: ommx::v1::SampleSet,
    descriptor_annotations: HashMap<String, String>,
) -> Result<crate::SampleSet> {
    ommx::artifact::merge_sample_set_annotations(&mut proto, &descriptor_annotations);
    Ok(crate::SampleSet {
        inner: proto.parse(&())?,
    })
}

fn replace_instance_annotations(
    instance: &mut ommx::v1::Instance,
    annotations: &HashMap<String, String>,
) {
    instance.description = None;
    instance.annotations.clear();
    ommx::artifact::overlay_instance_annotations(instance, annotations);
}

fn replace_parametric_instance_annotations(
    instance: &mut ommx::v1::ParametricInstance,
    annotations: &HashMap<String, String>,
) {
    instance.description = None;
    instance.annotations.clear();
    ommx::artifact::overlay_parametric_instance_annotations(instance, annotations);
}

fn replace_solution_annotations(
    solution: &mut ommx::v1::Solution,
    annotations: &HashMap<String, String>,
) {
    solution.metadata = None;
    solution.annotations.clear();
    ommx::artifact::overlay_solution_annotations(solution, annotations);
}

fn replace_sample_set_annotations(
    sample_set: &mut ommx::v1::SampleSet,
    annotations: &HashMap<String, String>,
) {
    sample_set.metadata = None;
    sample_set.annotations.clear();
    ommx::artifact::overlay_sample_set_annotations(sample_set, annotations);
}

fn readonly_annotations<'py>(
    py: pyo3::Python<'py>,
    annotations: HashMap<String, String>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::PyAny>> {
    let dict = pyo3::types::PyDict::new(py);
    for (key, value) in annotations {
        dict.set_item(key, value)?;
    }
    py.import("types")?
        .getattr("MappingProxyType")?
        .call1((dict,))
}

pub struct AnnotationMapping(HashMap<String, String>);

impl AnnotationMapping {
    pub fn new(annotations: HashMap<String, String>) -> Self {
        Self(annotations)
    }
}

impl<'py> pyo3::IntoPyObject<'py> for AnnotationMapping {
    type Target = pyo3::PyAny;
    type Output = pyo3::Bound<'py, pyo3::PyAny>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        readonly_annotations(py, self.0)
    }
}

impl pyo3_stub_gen::PyStubType for AnnotationMapping {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            import: ["types".into()].into(),
            name: "types.MappingProxyType[str, str]".into(),
            source_module: None,
            type_refs: Default::default(),
        }
    }
}

pub trait FlatAnnotations {
    fn flat_annotations(&self) -> HashMap<String, String>;
    fn set_flat_annotations(&mut self, annotations: HashMap<String, String>) -> Result<()>;

    fn insert_flat_annotation(&mut self, key: String, value: String) -> Result<()> {
        let mut annotations = self.flat_annotations();
        annotations.insert(key, value);
        self.set_flat_annotations(annotations)
    }
}

impl FlatAnnotations for crate::Instance {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let proto = instance_to_v1_with_annotations(self);
        ommx::artifact::instance_annotations(&proto)
    }

    fn set_flat_annotations(&mut self, annotations: HashMap<String, String>) -> Result<()> {
        let mut proto = instance_to_v1_with_annotations(self);
        replace_instance_annotations(&mut proto, &annotations);
        self.inner = proto.try_into()?;
        Ok(())
    }
}

impl FlatAnnotations for crate::ParametricInstance {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let proto = parametric_instance_to_v1_with_annotations(self);
        ommx::artifact::parametric_instance_annotations(&proto)
    }

    fn set_flat_annotations(&mut self, annotations: HashMap<String, String>) -> Result<()> {
        let mut proto = parametric_instance_to_v1_with_annotations(self);
        replace_parametric_instance_annotations(&mut proto, &annotations);
        self.inner = proto.parse(&())?;
        Ok(())
    }
}

impl FlatAnnotations for crate::Solution {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let proto = solution_to_v1_with_annotations(self);
        ommx::artifact::solution_annotations(&proto)
    }

    fn set_flat_annotations(&mut self, annotations: HashMap<String, String>) -> Result<()> {
        let mut proto = solution_to_v1_with_annotations(self);
        replace_solution_annotations(&mut proto, &annotations);
        self.inner = proto.parse(&())?;
        Ok(())
    }
}

impl FlatAnnotations for crate::SampleSet {
    fn flat_annotations(&self) -> HashMap<String, String> {
        let proto = sample_set_to_v1_with_annotations(self);
        ommx::artifact::sample_set_annotations(&proto)
    }

    fn set_flat_annotations(&mut self, annotations: HashMap<String, String>) -> Result<()> {
        let mut proto = sample_set_to_v1_with_annotations(self);
        replace_sample_set_annotations(&mut proto, &annotations);
        self.inner = proto.parse(&())?;
        Ok(())
    }
}

/// Implement all annotation properties for Instance / ParametricInstance pattern.
///
/// Generates a single `#[pymethods]` block with:
/// - annotations read-only getter and replace_annotations
/// - add_user_annotation, add_user_annotations, get_user_annotation, get_user_annotations
/// - title, license, dataset, authors (str list), num_variables, num_constraints (int), created (datetime)
macro_rules! impl_instance_annotations {
    ($ty:ty, $namespace:literal) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pyo3::pymethods]
        impl $ty {
            // --- Core annotation methods ---

            /// Returns a read-only mapping of flat annotations.
            ///
            /// Use {meth}`add_user_annotation`, metadata properties, or
            /// {meth}`replace_annotations` to modify annotations.
            #[getter]
            pub fn annotations(&self) -> $crate::annotations::AnnotationMapping {
                $crate::annotations::AnnotationMapping::new(
                    $crate::annotations::FlatAnnotations::flat_annotations(self),
                )
            }

            pub fn replace_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
            ) -> anyhow::Result<()> {
                $crate::annotations::FlatAnnotations::set_flat_annotations(self, annotations)
            }

            #[pyo3(signature = (key, value, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotation(
                &mut self,
                key: &str,
                value: &str,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<()> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                let full_key = format!("{ns}{key}");
                if full_key.starts_with("org.ommx.v1.") {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "User annotation key `{full_key}` is reserved for OMMX metadata"
                    )));
                }
                self.inner
                    .annotations
                    .insert(full_key, value.to_string());
                Ok(())
            }

            #[pyo3(signature = (annotations, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<()> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                let entries = annotations
                    .into_iter()
                    .map(|(key, value)| {
                        let full_key = format!("{ns}{key}");
                        if full_key.starts_with("org.ommx.v1.") {
                            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                                "User annotation key `{full_key}` is reserved for OMMX metadata"
                            )));
                        }
                        Ok((full_key, value))
                    })
                    .collect::<pyo3::PyResult<Vec<_>>>()?;
                for (key, value) in entries {
                    self.inner.annotations.insert(key, value);
                }
                Ok(())
            }

            #[pyo3(signature = (key, *, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotation(
                &self,
                key: &str,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<String> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                let full_key = format!("{ns}{key}");
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(&full_key)
                    .cloned()
                    .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(full_key))
            }

            #[pyo3(signature = (*, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotations(
                &self,
                annotation_namespace: &str,
            ) -> std::collections::HashMap<String, String> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .into_iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix(&ns)
                            .map(|stripped| (stripped.to_string(), value))
                    })
                    .collect()
            }

            // --- String properties ---

            #[getter]
            pub fn title(&self) -> Option<String> {
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(concat!($namespace, ".title"))
                    .cloned()
            }

            #[setter]
            pub fn set_title(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".title").to_string(),
                    value,
                )
            }

            #[getter]
            pub fn license(&self) -> Option<String> {
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(concat!($namespace, ".license"))
                    .cloned()
            }

            #[setter]
            pub fn set_license(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".license").to_string(),
                    value,
                )
            }

            #[getter]
            pub fn dataset(&self) -> Option<String> {
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(concat!($namespace, ".dataset"))
                    .cloned()
            }

            #[setter]
            pub fn set_dataset(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".dataset").to_string(),
                    value,
                )
            }

            // --- String list property ---

            #[getter]
            pub fn authors(&self) -> Vec<String> {
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(concat!($namespace, ".authors"))
                    .filter(|v| !v.is_empty())
                    .map(|v| v.split(',').map(|s| s.to_string()).collect())
                    .unwrap_or_default()
            }

            #[setter]
            pub fn set_authors(&mut self, value: Vec<String>) -> anyhow::Result<()> {
                let key = concat!($namespace, ".authors").to_string();
                let value = if value.is_empty() {
                    String::new()
                } else {
                    value.join(",")
                };
                $crate::annotations::FlatAnnotations::insert_flat_annotation(self, key, value)
            }

            // --- Integer properties ---

            #[getter]
            pub fn num_variables(&self) -> Option<i64> {
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(concat!($namespace, ".variables"))
                    .and_then(|v| v.parse().ok())
            }

            #[getter]
            pub fn num_constraints(&self) -> Option<i64> {
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(concat!($namespace, ".constraints"))
                    .and_then(|v| v.parse().ok())
            }

            // --- Datetime property (RFC3339 string ↔ Python datetime via dateutil) ---

            #[getter]
            pub fn created<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
                let annotations = $crate::annotations::FlatAnnotations::flat_annotations(self);
                let value = match annotations.get(concat!($namespace, ".created")) {
                    Some(v) if !v.is_empty() => v,
                    _ => return Ok(None),
                };
                let dateutil = py.import("dateutil.parser")?;
                let dt = dateutil.call_method1("isoparse", (value,))?;
                Ok(Some(dt.cast_into()?))
            }

            #[setter]
            pub fn set_created(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::types::PyDateTime>,
            ) -> pyo3::PyResult<()> {
                let iso: String = value.call_method0("isoformat")?.extract()?;
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".created").to_string(),
                    iso,
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
            }
        }
    };
}

/// Implement all annotation properties for Solution / SampleSet pattern.
///
/// Generates a single `#[pymethods]` block with:
/// - annotations read-only getter and replace_annotations
/// - add_user_annotation, add_user_annotations, get_user_annotation, get_user_annotations
/// - instance_digest, solver_annotation (json), parameters_annotation (json), start, end (datetime)
macro_rules! impl_solution_annotations {
    ($ty:ty, $namespace:literal) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pyo3::pymethods]
        impl $ty {
            // --- Core annotation methods ---

            /// Returns a read-only mapping of flat annotations.
            ///
            /// Use {meth}`add_user_annotation`, metadata properties, or
            /// {meth}`replace_annotations` to modify annotations.
            #[getter]
            pub fn annotations(&self) -> $crate::annotations::AnnotationMapping {
                $crate::annotations::AnnotationMapping::new(
                    $crate::annotations::FlatAnnotations::flat_annotations(self),
                )
            }

            pub fn replace_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
            ) -> anyhow::Result<()> {
                $crate::annotations::FlatAnnotations::set_flat_annotations(self, annotations)
            }

            #[pyo3(signature = (key, value, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotation(
                &mut self,
                key: &str,
                value: &str,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<()> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                let full_key = format!("{ns}{key}");
                if full_key.starts_with("org.ommx.v1.") {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "User annotation key `{full_key}` is reserved for OMMX metadata"
                    )));
                }
                self.inner
                    .annotations
                    .insert(full_key, value.to_string());
                Ok(())
            }

            #[pyo3(signature = (annotations, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<()> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                let entries = annotations
                    .into_iter()
                    .map(|(key, value)| {
                        let full_key = format!("{ns}{key}");
                        if full_key.starts_with("org.ommx.v1.") {
                            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                                "User annotation key `{full_key}` is reserved for OMMX metadata"
                            )));
                        }
                        Ok((full_key, value))
                    })
                    .collect::<pyo3::PyResult<Vec<_>>>()?;
                for (key, value) in entries {
                    self.inner.annotations.insert(key, value);
                }
                Ok(())
            }

            #[pyo3(signature = (key, *, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotation(
                &self,
                key: &str,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<String> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                let full_key = format!("{ns}{key}");
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(&full_key)
                    .cloned()
                    .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(full_key))
            }

            #[pyo3(signature = (*, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotations(
                &self,
                annotation_namespace: &str,
            ) -> std::collections::HashMap<String, String> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .into_iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix(&ns)
                            .map(|stripped| (stripped.to_string(), value))
                    })
                    .collect()
            }

            // --- String property: instance digest ---

            #[getter]
            pub fn instance_digest(&self) -> Option<String> {
                $crate::annotations::FlatAnnotations::flat_annotations(self)
                    .get(concat!($namespace, ".instance"))
                    .cloned()
            }

            #[setter]
            pub fn set_instance_digest(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".instance").to_string(),
                    value,
                )
            }

            // --- JSON properties (serde_json ↔ Python object) ---

            #[getter]
            pub fn solver_annotation<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let annotations = $crate::annotations::FlatAnnotations::flat_annotations(self);
                let value = match annotations.get(concat!($namespace, ".solver")) {
                    Some(v) if !v.is_empty() => v,
                    _ => return Ok(None),
                };
                let json_value: serde_json::Value = serde_json::from_str(value)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                let obj = serde_pyobject::to_pyobject(py, &json_value)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                Ok(Some(obj))
            }

            #[setter]
            pub fn set_solver_annotation(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let json_value: serde_json::Value = serde_pyobject::from_pyobject(value.clone())
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                let json_str = serde_json::to_string(&json_value)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".solver").to_string(),
                    json_str,
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
            }

            #[getter]
            pub fn parameters_annotation<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let annotations = $crate::annotations::FlatAnnotations::flat_annotations(self);
                let value = match annotations.get(concat!($namespace, ".parameters")) {
                    Some(v) if !v.is_empty() => v,
                    _ => return Ok(None),
                };
                let json_value: serde_json::Value = serde_json::from_str(value)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                let obj = serde_pyobject::to_pyobject(py, &json_value)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                Ok(Some(obj))
            }

            #[setter]
            pub fn set_parameters_annotation(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let json_value: serde_json::Value = serde_pyobject::from_pyobject(value.clone())
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                let json_str = serde_json::to_string(&json_value)
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".parameters").to_string(),
                    json_str,
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
            }

            // --- Datetime properties (RFC3339 string ↔ Python datetime via dateutil) ---

            #[getter]
            pub fn start<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
                let annotations = $crate::annotations::FlatAnnotations::flat_annotations(self);
                let value = match annotations.get(concat!($namespace, ".start")) {
                    Some(v) if !v.is_empty() => v,
                    _ => return Ok(None),
                };
                let dateutil = py.import("dateutil.parser")?;
                let dt = dateutil.call_method1("isoparse", (value,))?;
                Ok(Some(dt.cast_into()?))
            }

            #[setter]
            pub fn set_start(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::types::PyDateTime>,
            ) -> pyo3::PyResult<()> {
                let iso: String = value.call_method0("isoformat")?.extract()?;
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".start").to_string(),
                    iso,
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
            }

            #[getter]
            pub fn end<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
                let annotations = $crate::annotations::FlatAnnotations::flat_annotations(self);
                let value = match annotations.get(concat!($namespace, ".end")) {
                    Some(v) if !v.is_empty() => v,
                    _ => return Ok(None),
                };
                let dateutil = py.import("dateutil.parser")?;
                let dt = dateutil.call_method1("isoparse", (value,))?;
                Ok(Some(dt.cast_into()?))
            }

            #[setter]
            pub fn set_end(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::types::PyDateTime>,
            ) -> pyo3::PyResult<()> {
                let iso: String = value.call_method0("isoformat")?.extract()?;
                $crate::annotations::FlatAnnotations::insert_flat_annotation(
                    self,
                    concat!($namespace, ".end").to_string(),
                    iso,
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
            }

            // --- Backward-compatible aliases ---
            // Python SDK previously exposed these as `instance`, `solver`, `parameters`

            #[getter]
            #[pyo3(name = "instance")]
            pub fn instance_alias(&self) -> Option<String> {
                self.instance_digest()
            }

            #[setter]
            #[pyo3(name = "instance")]
            pub fn set_instance_alias(&mut self, value: String) -> anyhow::Result<()> {
                self.set_instance_digest(value)
            }

            #[getter]
            #[pyo3(name = "solver")]
            pub fn solver_alias<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                self.solver_annotation(py)
            }

            #[setter]
            #[pyo3(name = "solver")]
            pub fn set_solver_alias(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                self.set_solver_annotation(value)
            }

            #[getter]
            #[pyo3(name = "parameters")]
            pub fn parameters_alias<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                self.parameters_annotation(py)
            }

            #[setter]
            #[pyo3(name = "parameters")]
            pub fn set_parameters_alias(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                self.set_parameters_annotation(value)
            }
        }
    };
}
