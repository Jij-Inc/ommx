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

pub fn description_mut(
    description: &mut Option<ommx::v1::instance::Description>,
) -> &mut ommx::v1::instance::Description {
    description.get_or_insert_with(ommx::v1::instance::Description::default)
}

pub fn process_metadata_mut(
    metadata: &mut Option<ommx::v1::ProcessMetadata>,
) -> &mut ommx::v1::ProcessMetadata {
    metadata.get_or_insert_with(ommx::v1::ProcessMetadata::default)
}

pub fn user_annotation_key(key: &str, annotation_namespace: &str) -> pyo3::PyResult<String> {
    let ns = normalize_namespace(annotation_namespace);
    let full_key = format!("{ns}{key}");
    if ommx::is_reserved_annotation_key(&full_key) {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "User annotation key `{full_key}` is reserved for OMMX metadata"
        )));
    }
    Ok(full_key)
}

pub fn insert_user_annotation(
    annotations: &mut HashMap<String, String>,
    key: &str,
    value: &str,
    annotation_namespace: &str,
) -> pyo3::PyResult<()> {
    let key = user_annotation_key(key, annotation_namespace)?;
    annotations.insert(key, value.to_string());
    Ok(())
}

pub fn insert_user_annotations(
    target: &mut HashMap<String, String>,
    annotations: HashMap<String, String>,
    annotation_namespace: &str,
) -> pyo3::PyResult<()> {
    let entries = annotations
        .into_iter()
        .map(|(key, value)| {
            let key = user_annotation_key(&key, annotation_namespace)?;
            Ok((key, value))
        })
        .collect::<pyo3::PyResult<Vec<_>>>()?;
    for (key, value) in entries {
        target.insert(key, value);
    }
    Ok(())
}

pub fn get_user_annotation(
    annotations: &HashMap<String, String>,
    key: &str,
    annotation_namespace: &str,
) -> pyo3::PyResult<String> {
    let key = user_annotation_key(key, annotation_namespace)?;
    annotations
        .get(&key)
        .cloned()
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(key))
}

pub fn get_user_annotations(
    annotations: &HashMap<String, String>,
    annotation_namespace: &str,
) -> pyo3::PyResult<HashMap<String, String>> {
    let ns = normalize_namespace(annotation_namespace);
    if ommx::is_reserved_annotation_key(&ns) {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "User annotation namespace `{ns}` is reserved for OMMX metadata"
        )));
    }
    Ok(annotations
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix(&ns)
                .map(|stripped| (stripped.to_string(), value.clone()))
        })
        .collect())
}

pub fn datetime_from_rfc3339<'py>(
    py: pyo3::Python<'py>,
    value: Option<&String>,
) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let dateutil = py.import("dateutil.parser")?;
    let dt = dateutil.call_method1("isoparse", (value,))?;
    Ok(Some(dt.cast_into()?))
}

pub fn datetime_to_rfc3339(
    value: &pyo3::Bound<'_, pyo3::types::PyDateTime>,
) -> pyo3::PyResult<String> {
    value.call_method0("isoformat")?.extract()
}

pub fn json_string_to_py<'py>(
    py: pyo3::Python<'py>,
    value: Option<&String>,
) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let json_value: serde_json::Value = serde_json::from_str(value)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let obj = serde_pyobject::to_pyobject(py, &json_value)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(Some(obj))
}

pub fn py_to_json_string(value: &pyo3::Bound<'_, pyo3::PyAny>) -> pyo3::PyResult<String> {
    let json_value: serde_json::Value = serde_pyobject::from_pyobject(value.clone())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&json_value)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Implement annotation properties for Instance / ParametricInstance.
macro_rules! impl_instance_annotations {
    ($ty:ty) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pyo3::pymethods]
        impl $ty {
            /// Returns a read-only mapping of flat annotations.
            ///
            /// Use {meth}`add_user_annotation`, metadata properties, or
            /// {meth}`replace_annotations` to modify annotations.
            #[getter]
            pub fn annotations(&self) -> $crate::annotations::AnnotationMapping {
                $crate::annotations::AnnotationMapping::new($crate::annotations::flat_annotations(
                    &self.inner,
                ))
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
                $crate::annotations::insert_user_annotation(
                    &mut self.inner.annotations,
                    key,
                    value,
                    annotation_namespace,
                )
            }

            #[pyo3(signature = (annotations, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<()> {
                $crate::annotations::insert_user_annotations(
                    &mut self.inner.annotations,
                    annotations,
                    annotation_namespace,
                )
            }

            #[pyo3(signature = (key, *, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotation(
                &self,
                key: &str,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<String> {
                $crate::annotations::get_user_annotation(
                    &self.inner.annotations,
                    key,
                    annotation_namespace,
                )
            }

            #[pyo3(signature = (*, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotations(
                &self,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<std::collections::HashMap<String, String>> {
                $crate::annotations::get_user_annotations(
                    &self.inner.annotations,
                    annotation_namespace,
                )
            }

            #[getter]
            pub fn title(&self) -> Option<String> {
                self.inner
                    .description
                    .as_ref()
                    .and_then(|description| description.name.clone())
            }

            #[setter]
            pub fn set_title(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::description_mut(&mut self.inner.description).name =
                    Some(value);
                Ok(())
            }

            #[getter]
            pub fn license(&self) -> Option<String> {
                self.inner
                    .description
                    .as_ref()
                    .and_then(|description| description.license.clone())
            }

            #[setter]
            pub fn set_license(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::description_mut(&mut self.inner.description).license =
                    Some(value);
                Ok(())
            }

            #[getter]
            pub fn dataset(&self) -> Option<String> {
                self.inner
                    .description
                    .as_ref()
                    .and_then(|description| description.dataset.clone())
            }

            #[setter]
            pub fn set_dataset(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::description_mut(&mut self.inner.description).dataset =
                    Some(value);
                Ok(())
            }

            #[getter]
            pub fn authors(&self) -> Vec<String> {
                self.inner
                    .description
                    .as_ref()
                    .map(|description| description.authors.clone())
                    .unwrap_or_default()
            }

            #[setter]
            pub fn set_authors(&mut self, value: Vec<String>) -> anyhow::Result<()> {
                $crate::annotations::description_mut(&mut self.inner.description).authors = value;
                Ok(())
            }

            #[getter]
            pub fn num_variables(&self) -> Option<i64> {
                Some(self.inner.decision_variables().len() as i64)
            }

            #[getter]
            pub fn num_constraints(&self) -> Option<i64> {
                Some(self.inner.constraints().len() as i64)
            }

            #[getter]
            pub fn created<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
                $crate::annotations::datetime_from_rfc3339(
                    py,
                    self.inner
                        .description
                        .as_ref()
                        .and_then(|description| description.created.as_ref()),
                )
            }

            #[setter]
            pub fn set_created(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::types::PyDateTime>,
            ) -> pyo3::PyResult<()> {
                let value = $crate::annotations::datetime_to_rfc3339(value)?;
                $crate::annotations::description_mut(&mut self.inner.description).created =
                    Some(value);
                Ok(())
            }
        }
    };
}

/// Implement annotation properties for Solution / SampleSet.
macro_rules! impl_solution_annotations {
    ($ty:ty) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pyo3::pymethods]
        impl $ty {
            /// Returns a read-only mapping of flat annotations.
            ///
            /// Use {meth}`add_user_annotation`, metadata properties, or
            /// {meth}`replace_annotations` to modify annotations.
            #[getter]
            pub fn annotations(&self) -> $crate::annotations::AnnotationMapping {
                $crate::annotations::AnnotationMapping::new($crate::annotations::flat_annotations(
                    &self.inner,
                ))
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
                $crate::annotations::insert_user_annotation(
                    &mut self.inner.annotations,
                    key,
                    value,
                    annotation_namespace,
                )
            }

            #[pyo3(signature = (annotations, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<()> {
                $crate::annotations::insert_user_annotations(
                    &mut self.inner.annotations,
                    annotations,
                    annotation_namespace,
                )
            }

            #[pyo3(signature = (key, *, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotation(
                &self,
                key: &str,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<String> {
                $crate::annotations::get_user_annotation(
                    &self.inner.annotations,
                    key,
                    annotation_namespace,
                )
            }

            #[pyo3(signature = (*, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotations(
                &self,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<std::collections::HashMap<String, String>> {
                $crate::annotations::get_user_annotations(
                    &self.inner.annotations,
                    annotation_namespace,
                )
            }

            #[getter]
            pub fn instance_digest(&self) -> Option<String> {
                self.inner
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.instance.clone())
            }

            #[setter]
            pub fn set_instance_digest(&mut self, value: String) -> anyhow::Result<()> {
                $crate::annotations::process_metadata_mut(&mut self.inner.metadata).instance =
                    Some(value);
                Ok(())
            }

            #[getter]
            pub fn solver_annotation<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                $crate::annotations::json_string_to_py(
                    py,
                    self.inner
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.solver.as_ref()),
                )
            }

            #[setter]
            pub fn set_solver_annotation(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let value = $crate::annotations::py_to_json_string(value)?;
                $crate::annotations::process_metadata_mut(&mut self.inner.metadata).solver =
                    Some(value);
                Ok(())
            }

            #[getter]
            pub fn parameters_annotation<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                $crate::annotations::json_string_to_py(
                    py,
                    self.inner
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.parameters.as_ref()),
                )
            }

            #[setter]
            pub fn set_parameters_annotation(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let value = $crate::annotations::py_to_json_string(value)?;
                $crate::annotations::process_metadata_mut(&mut self.inner.metadata).parameters =
                    Some(value);
                Ok(())
            }

            #[getter]
            pub fn start<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
                $crate::annotations::datetime_from_rfc3339(
                    py,
                    self.inner
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.start.as_ref()),
                )
            }

            #[setter]
            pub fn set_start(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::types::PyDateTime>,
            ) -> pyo3::PyResult<()> {
                let value = $crate::annotations::datetime_to_rfc3339(value)?;
                $crate::annotations::process_metadata_mut(&mut self.inner.metadata).start =
                    Some(value);
                Ok(())
            }

            #[getter]
            pub fn end<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::types::PyDateTime>>> {
                $crate::annotations::datetime_from_rfc3339(
                    py,
                    self.inner
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.end.as_ref()),
                )
            }

            #[setter]
            pub fn set_end(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::types::PyDateTime>,
            ) -> pyo3::PyResult<()> {
                let value = $crate::annotations::datetime_to_rfc3339(value)?;
                $crate::annotations::process_metadata_mut(&mut self.inner.metadata).end =
                    Some(value);
                Ok(())
            }

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
