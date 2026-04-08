/// Normalize annotation namespace to ensure it ends with "."
pub fn normalize_namespace(ns: &str) -> String {
    if ns.ends_with('.') {
        ns.to_string()
    } else {
        format!("{ns}.")
    }
}

/// Implement all annotation properties for Instance / ParametricInstance pattern.
///
/// Generates a single `#[pymethods]` block with:
/// - annotations getter/setter
/// - add_user_annotation, add_user_annotations, get_user_annotation, get_user_annotations
/// - title, license, dataset, authors (str list), num_variables, num_constraints (int), created (datetime)
macro_rules! impl_instance_annotations {
    ($ty:ty, $namespace:literal) => {
        #[pyo3::pymethods]
        impl $ty {
            // --- Core annotation methods ---

            #[getter]
            pub fn annotations(&self) -> std::collections::HashMap<String, String> {
                self.annotations.clone()
            }

            #[setter]
            pub fn set_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
            ) {
                self.annotations = annotations;
            }

            #[pyo3(signature = (key, value, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotation(
                &mut self,
                key: &str,
                value: &str,
                annotation_namespace: &str,
            ) {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                self.annotations
                    .insert(format!("{ns}{key}"), value.to_string());
            }

            #[pyo3(signature = (annotations, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
                annotation_namespace: &str,
            ) {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                for (key, value) in annotations {
                    self.annotations.insert(format!("{ns}{key}"), value);
                }
            }

            #[pyo3(signature = (key, *, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotation(
                &self,
                key: &str,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<String> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                let full_key = format!("{ns}{key}");
                self.annotations
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
                self.annotations
                    .iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix(&ns)
                            .map(|stripped| (stripped.to_string(), value.clone()))
                    })
                    .collect()
            }

            // --- String properties ---

            #[getter]
            pub fn title(&self) -> Option<String> {
                self.annotations.get(concat!($namespace, ".title")).cloned()
            }

            #[setter]
            pub fn set_title(&mut self, value: String) {
                self.annotations
                    .insert(concat!($namespace, ".title").to_string(), value);
            }

            #[getter]
            pub fn license(&self) -> Option<String> {
                self.annotations
                    .get(concat!($namespace, ".license"))
                    .cloned()
            }

            #[setter]
            pub fn set_license(&mut self, value: String) {
                self.annotations
                    .insert(concat!($namespace, ".license").to_string(), value);
            }

            #[getter]
            pub fn dataset(&self) -> Option<String> {
                self.annotations
                    .get(concat!($namespace, ".dataset"))
                    .cloned()
            }

            #[setter]
            pub fn set_dataset(&mut self, value: String) {
                self.annotations
                    .insert(concat!($namespace, ".dataset").to_string(), value);
            }

            // --- String list property ---

            #[getter]
            pub fn authors(&self) -> Vec<String> {
                self.annotations
                    .get(concat!($namespace, ".authors"))
                    .map(|v| v.split(',').map(|s| s.to_string()).collect())
                    .unwrap_or_default()
            }

            #[setter]
            pub fn set_authors(&mut self, value: Vec<String>) {
                self.annotations
                    .insert(concat!($namespace, ".authors").to_string(), value.join(","));
            }

            // --- Integer properties ---

            #[getter]
            pub fn num_variables(&self) -> Option<i64> {
                self.annotations
                    .get(concat!($namespace, ".variables"))
                    .and_then(|v| v.parse().ok())
            }

            #[setter]
            pub fn set_num_variables(&mut self, value: i64) {
                self.annotations.insert(
                    concat!($namespace, ".variables").to_string(),
                    value.to_string(),
                );
            }

            #[getter]
            pub fn num_constraints(&self) -> Option<i64> {
                self.annotations
                    .get(concat!($namespace, ".constraints"))
                    .and_then(|v| v.parse().ok())
            }

            #[setter]
            pub fn set_num_constraints(&mut self, value: i64) {
                self.annotations.insert(
                    concat!($namespace, ".constraints").to_string(),
                    value.to_string(),
                );
            }

            // --- Datetime property ---

            #[getter]
            pub fn created<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let value = match self.annotations.get(concat!($namespace, ".created")) {
                    Some(v) => v,
                    None => return Ok(None),
                };
                let dateutil = py.import("dateutil.parser")?;
                let dt = dateutil.call_method1("isoparse", (value,))?;
                Ok(Some(dt))
            }

            #[setter]
            pub fn set_created(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let iso: String = value.call_method0("isoformat")?.extract()?;
                self.annotations
                    .insert(concat!($namespace, ".created").to_string(), iso);
                Ok(())
            }
        }
    };
}

/// Implement all annotation properties for Solution / SampleSet pattern.
///
/// Generates a single `#[pymethods]` block with:
/// - annotations getter/setter
/// - add_user_annotation, add_user_annotations, get_user_annotation, get_user_annotations
/// - instance_digest, solver_annotation (json), parameters_annotation (json), start, end (datetime)
macro_rules! impl_solution_annotations {
    ($ty:ty, $namespace:literal) => {
        #[pyo3::pymethods]
        impl $ty {
            // --- Core annotation methods ---

            #[getter]
            pub fn annotations(&self) -> std::collections::HashMap<String, String> {
                self.annotations.clone()
            }

            #[setter]
            pub fn set_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
            ) {
                self.annotations = annotations;
            }

            #[pyo3(signature = (key, value, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotation(
                &mut self,
                key: &str,
                value: &str,
                annotation_namespace: &str,
            ) {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                self.annotations
                    .insert(format!("{ns}{key}"), value.to_string());
            }

            #[pyo3(signature = (annotations, *, annotation_namespace = "org.ommx.user."))]
            pub fn add_user_annotations(
                &mut self,
                annotations: std::collections::HashMap<String, String>,
                annotation_namespace: &str,
            ) {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                for (key, value) in annotations {
                    self.annotations.insert(format!("{ns}{key}"), value);
                }
            }

            #[pyo3(signature = (key, *, annotation_namespace = "org.ommx.user."))]
            pub fn get_user_annotation(
                &self,
                key: &str,
                annotation_namespace: &str,
            ) -> pyo3::PyResult<String> {
                let ns = $crate::annotations::normalize_namespace(annotation_namespace);
                let full_key = format!("{ns}{key}");
                self.annotations
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
                self.annotations
                    .iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix(&ns)
                            .map(|stripped| (stripped.to_string(), value.clone()))
                    })
                    .collect()
            }

            // --- String property: instance digest ---

            #[getter]
            pub fn instance_digest(&self) -> Option<String> {
                self.annotations
                    .get(concat!($namespace, ".instance"))
                    .cloned()
            }

            #[setter]
            pub fn set_instance_digest(&mut self, value: String) {
                self.annotations
                    .insert(concat!($namespace, ".instance").to_string(), value);
            }

            // --- JSON properties ---

            #[getter]
            pub fn solver_annotation<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let value = match self.annotations.get(concat!($namespace, ".solver")) {
                    Some(v) => v,
                    None => return Ok(None),
                };
                let json_mod = py.import("json")?;
                let obj = json_mod.call_method1("loads", (value,))?;
                Ok(Some(obj))
            }

            #[setter]
            pub fn set_solver_annotation<'py>(
                &mut self,
                value: &pyo3::Bound<'py, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py = value.py();
                let json_mod = py.import("json")?;
                let json_str: String = json_mod.call_method1("dumps", (value,))?.extract()?;
                self.annotations
                    .insert(concat!($namespace, ".solver").to_string(), json_str);
                Ok(())
            }

            #[getter]
            pub fn parameters_annotation<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let value = match self.annotations.get(concat!($namespace, ".parameters")) {
                    Some(v) => v,
                    None => return Ok(None),
                };
                let json_mod = py.import("json")?;
                let obj = json_mod.call_method1("loads", (value,))?;
                Ok(Some(obj))
            }

            #[setter]
            pub fn set_parameters_annotation<'py>(
                &mut self,
                value: &pyo3::Bound<'py, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py = value.py();
                let json_mod = py.import("json")?;
                let json_str: String = json_mod.call_method1("dumps", (value,))?.extract()?;
                self.annotations
                    .insert(concat!($namespace, ".parameters").to_string(), json_str);
                Ok(())
            }

            // --- Datetime properties ---

            #[getter]
            pub fn start<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let value = match self.annotations.get(concat!($namespace, ".start")) {
                    Some(v) => v,
                    None => return Ok(None),
                };
                let dateutil = py.import("dateutil.parser")?;
                let dt = dateutil.call_method1("isoparse", (value,))?;
                Ok(Some(dt))
            }

            #[setter]
            pub fn set_start(
                &mut self,
                value: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let iso: String = value.call_method0("isoformat")?.extract()?;
                self.annotations
                    .insert(concat!($namespace, ".start").to_string(), iso);
                Ok(())
            }

            #[getter]
            pub fn end<'py>(
                &self,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                let value = match self.annotations.get(concat!($namespace, ".end")) {
                    Some(v) => v,
                    None => return Ok(None),
                };
                let dateutil = py.import("dateutil.parser")?;
                let dt = dateutil.call_method1("isoparse", (value,))?;
                Ok(Some(dt))
            }

            #[setter]
            pub fn set_end(&mut self, value: &pyo3::Bound<'_, pyo3::PyAny>) -> pyo3::PyResult<()> {
                let iso: String = value.call_method0("isoformat")?.extract()?;
                self.annotations
                    .insert(concat!($namespace, ".end").to_string(), iso);
                Ok(())
            }
        }
    };
}

pub(crate) use impl_instance_annotations;
pub(crate) use impl_solution_annotations;
