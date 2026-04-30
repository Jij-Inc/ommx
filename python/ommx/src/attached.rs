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
/// metadata stores under the same method names (e.g. `constraint_metadata()` /
/// `constraint_metadata_mut()`), so generated metadata methods can dispatch on
/// the variant without changing the call shape.
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
/// `id: $ID` field. `$get` / `$get_mut` are the matching method names exposed
/// by both `Instance` and `ParametricInstance` for accessing the relevant
/// `ConstraintMetadataStore<$ID>` (e.g. `constraint_metadata` /
/// `constraint_metadata_mut`).
///
/// The macro emits `name` / `subscripts` / `description` / `parameters` /
/// `provenance` getters and the corresponding `set_*` / `add_*` write-through
/// setters in a separate `#[pymethods]` block, so call sites can keep
/// kind-specific accessors in their own `#[pymethods]` block.
#[macro_export]
macro_rules! attached_metadata_methods {
    ($Self:ident, $ID:ty, $get:ident, $get_mut:ident) => {
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
            pub fn set_name(&self, py: pyo3::Python<'_>, name: String) {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        p.borrow_mut(py).inner.$get_mut().set_name(self.id, name)
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        p.borrow_mut(py).inner.$get_mut().set_name(self.id, name)
                    }
                }
            }

            /// Alias for {meth}`set_name` (backward compatibility).
            pub fn add_name(&self, py: pyo3::Python<'_>, name: String) {
                self.set_name(py, name);
            }

            /// Set the subscripts. Writes through to the parent host's SoA metadata store.
            pub fn set_subscripts(&self, py: pyo3::Python<'_>, subscripts: Vec<i64>) {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .set_subscripts(self.id, subscripts),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .set_subscripts(self.id, subscripts),
                }
            }

            /// Append subscripts. Writes through to the parent host's SoA metadata store.
            pub fn add_subscripts(&self, py: pyo3::Python<'_>, subscripts: Vec<i64>) {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .extend_subscripts(self.id, subscripts),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .extend_subscripts(self.id, subscripts),
                }
            }

            /// Set the description. Writes through to the parent host's SoA metadata store.
            pub fn set_description(&self, py: pyo3::Python<'_>, description: String) {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .set_description(self.id, description),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .set_description(self.id, description),
                }
            }

            /// Alias for {meth}`set_description` (backward compatibility).
            pub fn add_description(&self, py: pyo3::Python<'_>, description: String) {
                self.set_description(py, description);
            }

            /// Replace all parameters. Writes through to the parent host's SoA metadata store.
            pub fn set_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) {
                let params: fnv::FnvHashMap<String, String> = parameters.into_iter().collect();
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .set_parameters(self.id, params),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .set_parameters(self.id, params),
                }
            }

            /// Alias for {meth}`set_parameters` (backward compatibility).
            pub fn add_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) {
                self.set_parameters(py, parameters);
            }

            /// Add a single parameter entry. Writes through to the parent host's SoA metadata store.
            pub fn add_parameter(&self, py: pyo3::Python<'_>, key: String, value: String) {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .set_parameter(self.id, key, value),
                    $crate::ConstraintHost::Parametric(p) => p
                        .borrow_mut(py)
                        .inner
                        .$get_mut()
                        .set_parameter(self.id, key, value),
                }
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
