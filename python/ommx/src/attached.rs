//! Shared infrastructure for `AttachedX` write-through wrappers.
//!
//! The `AttachedConstraint`, `AttachedDecisionVariable`,
//! `AttachedIndicatorConstraint`, etc. pyclasses each hold a back-reference to
//! their parent host (either an `Instance` or a `ParametricInstance`) plus an
//! id. They share the same metadata read / write surface — the ID type and the
//! collection-level metadata accessor differ, but the field set
//! (`name` / `subscripts` / `description` / `parameters` / `provenance`) is
//! identical.
//!
//! [`ConstraintHost`] models the host fork as an enum so a single Python class
//! per kind covers both hosts, and [`attached_metadata_methods!`] stamps out
//! the metadata getter / setter boilerplate against a caller-supplied metadata
//! accessor pair.

use crate::{Instance, ParametricInstance};
use pyo3::prelude::*;

/// Back-reference to the parent host of an `AttachedX` write-through wrapper.
///
/// Both [`Instance`] and [`ParametricInstance`] expose collection-level
/// metadata stores under the same getter names (e.g. `constraint_metadata()`)
/// and owner-checked setters (e.g. `set_constraint_metadata(...)`), so generated
/// metadata methods can dispatch on the variant without changing the call shape.
pub enum ConstraintHost {
    Instance(Py<Instance>),
    Parametric(Py<ParametricInstance>),
}

impl ConstraintHost {
    /// Refcount-bump the inner `Py<...>` handle.
    pub fn clone_ref(&self, py: Python<'_>) -> Self {
        match self {
            ConstraintHost::Instance(p) => ConstraintHost::Instance(p.clone_ref(py)),
            ConstraintHost::Parametric(p) => ConstraintHost::Parametric(p.clone_ref(py)),
        }
    }
}

/// Generate the metadata getters and setters for an `AttachedX` pyclass.
///
/// Expects the wrapper struct to have a `host: ConstraintHost` field and an
/// `id: $ID` field. `$get` / `$set` are the matching method names exposed
/// by both `Instance` and `ParametricInstance` for reading the relevant
/// `ConstraintMetadataStore<$ID>` and replacing one ID's metadata (e.g.
/// `constraint_metadata` / `set_constraint_metadata`).
///
/// The macro emits `name` / `subscripts` / `description` / `parameters` /
/// `provenance` getters and the corresponding `set_*` / `add_*` write-through
/// setters in a separate `#[pymethods]` block, so call sites can keep
/// kind-specific accessors in their own `#[pymethods]` block.
#[macro_export]
macro_rules! attached_metadata_methods {
    ($Self:ident, $ID:ty, $get:ident, $set:ident) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $Self {
            #[getter]
            pub fn name(&self, py: pyo3::Python<'_>) -> Option<String> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        p.borrow(py).inner.$get().name(self.id).map(str::to_owned)
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        p.borrow(py).inner.$get().name(self.id).map(str::to_owned)
                    }
                }
            }

            #[getter]
            pub fn subscripts(&self, py: pyo3::Python<'_>) -> Vec<i64> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        p.borrow(py).inner.$get().subscripts(self.id).to_vec()
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        p.borrow(py).inner.$get().subscripts(self.id).to_vec()
                    }
                }
            }

            #[getter]
            pub fn description(&self, py: pyo3::Python<'_>) -> Option<String> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow(py)
                        .inner
                        .$get()
                        .description(self.id)
                        .map(str::to_owned),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow(py)
                        .inner
                        .$get()
                        .description(self.id)
                        .map(str::to_owned),
                }
            }

            #[getter]
            pub fn parameters(
                &self,
                py: pyo3::Python<'_>,
            ) -> std::collections::HashMap<String, String> {
                let collect = |params: &fnv::FnvHashMap<String, String>| {
                    params
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<std::collections::HashMap<_, _>>()
                };
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        collect(p.borrow(py).inner.$get().parameters(self.id))
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        collect(p.borrow(py).inner.$get().parameters(self.id))
                    }
                }
            }

            /// Set the name. Writes through to the parent host's SoA metadata store.
            pub fn set_name(&self, py: pyo3::Python<'_>, name: String) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.name = Some(name);
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.name = Some(name);
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            /// Alias for {meth}`set_name` (backward compatibility).
            pub fn add_name(&self, py: pyo3::Python<'_>, name: String) -> anyhow::Result<()> {
                self.set_name(py, name)
            }

            /// Set the subscripts. Writes through to the parent host's SoA metadata store.
            pub fn set_subscripts(
                &self,
                py: pyo3::Python<'_>,
                subscripts: Vec<i64>,
            ) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.subscripts = subscripts;
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.subscripts = subscripts;
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            /// Append subscripts. Writes through to the parent host's SoA metadata store.
            pub fn add_subscripts(
                &self,
                py: pyo3::Python<'_>,
                subscripts: Vec<i64>,
            ) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.subscripts.extend(subscripts);
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.subscripts.extend(subscripts);
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            /// Set the description. Writes through to the parent host's SoA metadata store.
            pub fn set_description(
                &self,
                py: pyo3::Python<'_>,
                description: String,
            ) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.description = Some(description);
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.description = Some(description);
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            /// Alias for {meth}`set_description` (backward compatibility).
            pub fn add_description(
                &self,
                py: pyo3::Python<'_>,
                description: String,
            ) -> anyhow::Result<()> {
                self.set_description(py, description)
            }

            /// Replace all parameters. Writes through to the parent host's SoA metadata store.
            pub fn set_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) -> anyhow::Result<()> {
                let params: fnv::FnvHashMap<String, String> = parameters.into_iter().collect();
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.parameters = params;
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.parameters = params;
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            /// Alias for {meth}`set_parameters` (backward compatibility).
            pub fn add_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) -> anyhow::Result<()> {
                self.set_parameters(py, parameters)
            }

            /// Add a single parameter entry. Writes through to the parent host's SoA metadata store.
            pub fn add_parameter(
                &self,
                py: pyo3::Python<'_>,
                key: String,
                value: String,
            ) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.parameters.insert(key, value);
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.parameters.insert(key, value);
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            #[getter]
            pub fn provenance(&self, py: pyo3::Python<'_>) -> Vec<$crate::Provenance> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow(py)
                        .inner
                        .$get()
                        .provenance(self.id)
                        .iter()
                        .map($crate::Provenance::from)
                        .collect(),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow(py)
                        .inner
                        .$get()
                        .provenance(self.id)
                        .iter()
                        .map($crate::Provenance::from)
                        .collect(),
                }
            }
        }
    };
}

/// Like [`attached_metadata_methods!`] but for hosts with a
/// [`VariableMetadataStore`](ommx::VariableMetadataStore), which lacks a
/// `provenance` field. The store API is otherwise identical on reads; writes
/// go back through the host's checked `set_*_metadata` method.
///
/// `name` and `description` getters mirror [`DecisionVariable`](crate::DecisionVariable)
/// — they return `String` with the empty string for missing entries, *not*
/// `Option<String>`. This keeps the snapshot wrapper and the attached handle
/// in sync (`instance.decision_variables[i].name` and
/// `instance.attached_decision_variable(id).name` produce the same value
/// for the same id).
#[macro_export]
macro_rules! attached_variable_metadata_methods {
    ($Self:ident, $get:ident, $set:ident) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $Self {
            #[getter]
            pub fn name(&self, py: pyo3::Python<'_>) -> String {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow(py)
                        .inner
                        .$get()
                        .name(self.id)
                        .map(str::to_owned)
                        .unwrap_or_default(),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow(py)
                        .inner
                        .$get()
                        .name(self.id)
                        .map(str::to_owned)
                        .unwrap_or_default(),
                }
            }

            #[getter]
            pub fn subscripts(&self, py: pyo3::Python<'_>) -> Vec<i64> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        p.borrow(py).inner.$get().subscripts(self.id).to_vec()
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        p.borrow(py).inner.$get().subscripts(self.id).to_vec()
                    }
                }
            }

            #[getter]
            pub fn description(&self, py: pyo3::Python<'_>) -> String {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow(py)
                        .inner
                        .$get()
                        .description(self.id)
                        .map(str::to_owned)
                        .unwrap_or_default(),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow(py)
                        .inner
                        .$get()
                        .description(self.id)
                        .map(str::to_owned)
                        .unwrap_or_default(),
                }
            }

            #[getter]
            pub fn parameters(
                &self,
                py: pyo3::Python<'_>,
            ) -> std::collections::HashMap<String, String> {
                let collect = |params: &fnv::FnvHashMap<String, String>| {
                    params
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<std::collections::HashMap<_, _>>()
                };
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        collect(p.borrow(py).inner.$get().parameters(self.id))
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        collect(p.borrow(py).inner.$get().parameters(self.id))
                    }
                }
            }

            /// Set the name. Writes through to the parent host's SoA metadata store.
            pub fn set_name(&self, py: pyo3::Python<'_>, name: String) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.name = Some(name);
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.name = Some(name);
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            pub fn add_name(&self, py: pyo3::Python<'_>, name: String) -> anyhow::Result<()> {
                self.set_name(py, name)
            }

            pub fn set_subscripts(
                &self,
                py: pyo3::Python<'_>,
                subscripts: Vec<i64>,
            ) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.subscripts = subscripts;
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.subscripts = subscripts;
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            pub fn add_subscripts(
                &self,
                py: pyo3::Python<'_>,
                subscripts: Vec<i64>,
            ) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.subscripts.extend(subscripts);
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.subscripts.extend(subscripts);
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            pub fn set_description(
                &self,
                py: pyo3::Python<'_>,
                description: String,
            ) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.description = Some(description);
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.description = Some(description);
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            pub fn add_description(
                &self,
                py: pyo3::Python<'_>,
                description: String,
            ) -> anyhow::Result<()> {
                self.set_description(py, description)
            }

            pub fn set_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) -> anyhow::Result<()> {
                let params: fnv::FnvHashMap<String, String> = parameters.into_iter().collect();
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.parameters = params;
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.parameters = params;
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }

            pub fn add_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) -> anyhow::Result<()> {
                self.set_parameters(py, parameters)
            }

            pub fn add_parameter(
                &self,
                py: pyo3::Python<'_>,
                key: String,
                value: String,
            ) -> anyhow::Result<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.parameters.insert(key, value);
                        host.inner.$set(self.id, metadata)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut metadata = host.inner.$get().collect_for(self.id);
                        metadata.parameters.insert(key, value);
                        host.inner.$set(self.id, metadata)?;
                    }
                }
                Ok(())
            }
        }
    };
}
