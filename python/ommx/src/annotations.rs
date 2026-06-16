use anyhow::Result;
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

pub fn flat_annotations<T: ommx::FlatAnnotations>(value: &T) -> HashMap<String, String> {
    ommx::FlatAnnotations::flat_annotations(value)
}

pub fn replace_annotations<T: ommx::FlatAnnotations>(
    value: &mut T,
    annotations: HashMap<String, String>,
) -> Result<()> {
    ommx::FlatAnnotations::replace_annotations(value, annotations);
    Ok(())
}

pub fn insert_flat_annotation<T: ommx::FlatAnnotations>(
    value: &mut T,
    key: String,
    annotation_value: String,
) -> Result<()> {
    ommx::FlatAnnotations::insert_flat_annotation(value, key, annotation_value);
    Ok(())
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
                    $crate::annotations::flat_annotations(&self.inner),
                )
            }

            pub fn replace_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
            ) -> anyhow::Result<()> {
                $crate::annotations::replace_annotations(&mut self.inner, annotations)
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
                if ommx::is_reserved_annotation_key(&full_key) {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "User annotation key `{full_key}` is reserved for OMMX metadata"
                    )));
                }
                $crate::annotations::insert_flat_annotation(
                    &mut self.inner,
                    full_key,
                    value.to_string(),
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
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
                        if ommx::is_reserved_annotation_key(&full_key) {
                            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                                "User annotation key `{full_key}` is reserved for OMMX metadata"
                            )));
                        }
                        Ok((full_key, value))
                    })
                    .collect::<pyo3::PyResult<Vec<_>>>()?;
                for (key, value) in entries {
                    $crate::annotations::insert_flat_annotation(&mut self.inner, key, value)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
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
                $crate::annotations::flat_annotations(&self.inner)
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
                $crate::annotations::flat_annotations(&self.inner)
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
                $crate::annotations::flat_annotations(&self.inner)
                    .get(concat!($namespace, ".title"))
                    .cloned()
            }

            #[setter]
            pub fn set_title(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".title").to_string(),
                    value,
                )
            }

            #[getter]
            pub fn license(&self) -> Option<String> {
                $crate::annotations::flat_annotations(&self.inner)
                    .get(concat!($namespace, ".license"))
                    .cloned()
            }

            #[setter]
            pub fn set_license(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".license").to_string(),
                    value,
                )
            }

            #[getter]
            pub fn dataset(&self) -> Option<String> {
                $crate::annotations::flat_annotations(&self.inner)
                    .get(concat!($namespace, ".dataset"))
                    .cloned()
            }

            #[setter]
            pub fn set_dataset(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".dataset").to_string(),
                    value,
                )
            }

            // --- String list property ---

            #[getter]
            pub fn authors(&self) -> Vec<String> {
                $crate::annotations::flat_annotations(&self.inner)
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
                $crate::annotations::insert_flat_annotation(&mut self.inner, key, value)
            }

            // --- Integer properties ---

            #[getter]
            pub fn num_variables(&self) -> Option<i64> {
                $crate::annotations::flat_annotations(&self.inner)
                    .get(concat!($namespace, ".variables"))
                    .and_then(|v| v.parse().ok())
            }

            #[getter]
            pub fn num_constraints(&self) -> Option<i64> {
                $crate::annotations::flat_annotations(&self.inner)
                    .get(concat!($namespace, ".constraints"))
                    .and_then(|v| v.parse().ok())
            }

            // --- Datetime property (RFC3339 string ↔ Python datetime via dateutil) ---

            #[getter]
            pub fn created<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
                let annotations = $crate::annotations::flat_annotations(&self.inner);
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
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".created").to_string(),
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
                    $crate::annotations::flat_annotations(&self.inner),
                )
            }

            pub fn replace_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
            ) -> anyhow::Result<()> {
                $crate::annotations::replace_annotations(&mut self.inner, annotations)
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
                if ommx::is_reserved_annotation_key(&full_key) {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "User annotation key `{full_key}` is reserved for OMMX metadata"
                    )));
                }
                $crate::annotations::insert_flat_annotation(
                    &mut self.inner,
                    full_key,
                    value.to_string(),
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
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
                        if ommx::is_reserved_annotation_key(&full_key) {
                            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                                "User annotation key `{full_key}` is reserved for OMMX metadata"
                            )));
                        }
                        Ok((full_key, value))
                    })
                    .collect::<pyo3::PyResult<Vec<_>>>()?;
                for (key, value) in entries {
                    $crate::annotations::insert_flat_annotation(&mut self.inner, key, value)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
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
                $crate::annotations::flat_annotations(&self.inner)
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
                $crate::annotations::flat_annotations(&self.inner)
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
                $crate::annotations::flat_annotations(&self.inner)
                    .get(concat!($namespace, ".instance"))
                    .cloned()
            }

            #[setter]
            pub fn set_instance_digest(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".instance").to_string(),
                    value,
                )
            }

            // --- JSON properties (serde_json ↔ Python object) ---

            #[getter]
            pub fn solver_annotation<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let annotations = $crate::annotations::flat_annotations(&self.inner);
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
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".solver").to_string(),
                    json_str,
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
            }

            #[getter]
            pub fn parameters_annotation<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let annotations = $crate::annotations::flat_annotations(&self.inner);
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
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".parameters").to_string(),
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
                let annotations = $crate::annotations::flat_annotations(&self.inner);
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
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".start").to_string(),
                    iso,
                )
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
            }

            #[getter]
            pub fn end<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
                let annotations = $crate::annotations::flat_annotations(&self.inner);
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
                $crate::annotations::insert_flat_annotation(&mut self.inner, concat!($namespace, ".end").to_string(),
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
