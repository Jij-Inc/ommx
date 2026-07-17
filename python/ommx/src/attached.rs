//! Shared infrastructure for `AttachedX` write-through wrappers.
//!
//! The `AttachedConstraint`, `AttachedDecisionVariable`,
//! `AttachedIndicatorConstraint`, etc. pyclasses each hold a back-reference to
//! their parent host (either an `Instance` or a `ParametricInstance`) plus an
//! id. They share the same context read / write surface — the ID type and the
//! collection-level context accessor differ, but the field set
//! (`name` / `subscripts` / `description` / `parameters` / `provenance`) is
//! identical.
//!
//! [`ConstraintHost`] models the host fork as an enum so a single Python class
//! per kind covers both hosts, and [`attached_constraint_context_methods!`]
//! stamps out the getter / setter boilerplate against a caller-supplied context
//! accessor pair.

use crate::{Instance, ParametricInstance};
use pyo3::prelude::*;

/// Back-reference to the parent host of an `AttachedX` write-through wrapper.
///
/// Both [`Instance`] and [`ParametricInstance`] expose collection-level
/// context stores under the same getter names (e.g. `constraint_context()`)
/// and owner-checked setters (e.g. `set_constraint_context(...)`), so generated
/// context methods can dispatch on the variant without changing the call shape.
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

/// Generate the constraint-context getters and setters for an `AttachedX` pyclass.
///
/// Expects the wrapper struct to have a `host: ConstraintHost` field and an
/// `id: $ID` field. `$get` / `$set` are the matching method names exposed
/// by both `Instance` and `ParametricInstance` for reading the relevant
/// `ConstraintContextStore<$ID>` and replacing one ID's context (e.g.
/// `constraint_context` / `set_constraint_context`).
///
/// The macro emits `name` / `subscripts` / `description` / `parameters` /
/// `provenance` getters and the corresponding `set_*` / `add_*` write-through
/// setters in a separate `#[pymethods]` block, so call sites can keep
/// kind-specific accessors in their own `#[pymethods]` block.
#[macro_export]
macro_rules! attached_constraint_context_methods {
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

            /// Set the name. Writes through to the parent host's context store.
            pub fn set_name(
                &self,
                py: pyo3::Python<'_>,
                name: String,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.name = Some(name);
                        host.inner.$set(self.id, context)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.name = Some(name);
                        host.inner.$set(self.id, context)?;
                    }
                }
                Ok(())
            }

            /// Set the subscripts. Writes through to the parent host's context store.
            pub fn set_subscripts(
                &self,
                py: pyo3::Python<'_>,
                subscripts: Vec<i64>,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.subscripts = subscripts;
                        host.inner.$set(self.id, context)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.subscripts = subscripts;
                        host.inner.$set(self.id, context)?;
                    }
                }
                Ok(())
            }

            /// Append subscripts. Writes through to the parent host's context store.
            pub fn add_subscripts(
                &self,
                py: pyo3::Python<'_>,
                subscripts: Vec<i64>,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.subscripts.extend(subscripts);
                        host.inner.$set(self.id, context)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.subscripts.extend(subscripts);
                        host.inner.$set(self.id, context)?;
                    }
                }
                Ok(())
            }

            /// Set the description. Writes through to the parent host's context store.
            pub fn set_description(
                &self,
                py: pyo3::Python<'_>,
                description: String,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.description = Some(description);
                        host.inner.$set(self.id, context)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.description = Some(description);
                        host.inner.$set(self.id, context)?;
                    }
                }
                Ok(())
            }

            /// Replace all parameters. Writes through to the parent host's context store.
            pub fn set_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) -> $crate::error::OmmxPyResult<()> {
                let params: fnv::FnvHashMap<String, String> = parameters.into_iter().collect();
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.parameters = params;
                        host.inner.$set(self.id, context)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.parameters = params;
                        host.inner.$set(self.id, context)?;
                    }
                }
                Ok(())
            }

            /// Add parameter entries. Writes through to the parent host's context store.
            ///
            /// Existing keys are overwritten, and keys not mentioned in `parameters`
            /// are preserved. Use `set_parameters` to replace the whole map.
            pub fn add_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.parameters.extend(parameters);
                        host.inner.$set(self.id, context)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.parameters.extend(parameters);
                        host.inner.$set(self.id, context)?;
                    }
                }
                Ok(())
            }

            /// Add a single parameter entry. Writes through to the parent host's context store.
            pub fn add_parameter(
                &self,
                py: pyo3::Python<'_>,
                key: String,
                value: String,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.parameters.insert(key, value);
                        host.inner.$set(self.id, context)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut context = host.inner.$get().collect_for(self.id);
                        context.label.parameters.insert(key, value);
                        host.inner.$set(self.id, context)?;
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

/// Like [`attached_constraint_context_methods!`] but for hosts with a
/// [`VariableLabelStore`](ommx::VariableLabelStore), which lacks a
/// `provenance` field. The store API is otherwise identical on reads; writes
/// go back through the host's checked `set_*_label` method.
///
/// `name` and `description` getters mirror [`DecisionVariable`](crate::DecisionVariable)
/// — they return `String` with the empty string for missing entries, *not*
/// `Option<String>`. This keeps the snapshot wrapper and the attached handle
/// in sync (`instance.decision_variables[i].name` and
/// `instance.attached_decision_variable(id).name` produce the same value
/// for the same id).
#[macro_export]
macro_rules! attached_variable_labels_methods {
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

            /// Set the name. Writes through to the parent host's label store.
            pub fn set_name(
                &self,
                py: pyo3::Python<'_>,
                name: String,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.name = Some(name);
                        host.inner.$set(self.id, label)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.name = Some(name);
                        host.inner.$set(self.id, label)?;
                    }
                }
                Ok(())
            }

            pub fn set_subscripts(
                &self,
                py: pyo3::Python<'_>,
                subscripts: Vec<i64>,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.subscripts = subscripts;
                        host.inner.$set(self.id, label)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.subscripts = subscripts;
                        host.inner.$set(self.id, label)?;
                    }
                }
                Ok(())
            }

            pub fn add_subscripts(
                &self,
                py: pyo3::Python<'_>,
                subscripts: Vec<i64>,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.subscripts.extend(subscripts);
                        host.inner.$set(self.id, label)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.subscripts.extend(subscripts);
                        host.inner.$set(self.id, label)?;
                    }
                }
                Ok(())
            }

            pub fn set_description(
                &self,
                py: pyo3::Python<'_>,
                description: String,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.description = Some(description);
                        host.inner.$set(self.id, label)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.description = Some(description);
                        host.inner.$set(self.id, label)?;
                    }
                }
                Ok(())
            }

            pub fn set_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) -> $crate::error::OmmxPyResult<()> {
                let params: fnv::FnvHashMap<String, String> = parameters.into_iter().collect();
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.parameters = params;
                        host.inner.$set(self.id, label)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.parameters = params;
                        host.inner.$set(self.id, label)?;
                    }
                }
                Ok(())
            }

            /// Add parameter entries. Writes through to the parent host's label store.
            ///
            /// Existing keys are overwritten, and keys not mentioned in `parameters`
            /// are preserved. Use `set_parameters` to replace the whole map.
            pub fn add_parameters(
                &self,
                py: pyo3::Python<'_>,
                parameters: std::collections::HashMap<String, String>,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.parameters.extend(parameters);
                        host.inner.$set(self.id, label)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.parameters.extend(parameters);
                        host.inner.$set(self.id, label)?;
                    }
                }
                Ok(())
            }

            pub fn add_parameter(
                &self,
                py: pyo3::Python<'_>,
                key: String,
                value: String,
            ) -> $crate::error::OmmxPyResult<()> {
                match &self.host {
                    $crate::ConstraintHost::Instance(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.parameters.insert(key, value);
                        host.inner.$set(self.id, label)?;
                    }
                    $crate::ConstraintHost::Parametric(p) => {
                        let mut host = p.borrow_mut(py);
                        let mut label = host.inner.$get().collect_for(self.id);
                        label.parameters.insert(key, value);
                        host.inner.$set(self.id, label)?;
                    }
                }
                Ok(())
            }
        }
    };
}
