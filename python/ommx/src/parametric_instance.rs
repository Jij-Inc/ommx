use crate::{
    pandas::{
        constraint_id_col, constraint_kind_collection, entries_to_dataframe, ConstraintKind,
        PyDataFrame, ToPandasEntry,
    },
    Constraint, DecisionVariable, Function, Instance, NamedFunction, Parameter, RemovedConstraint,
    Sense,
};
use anyhow::Result;
use ommx::{ConstraintID, NamedFunctionID, VariableID};
use pyo3::{exceptions::PyKeyError, prelude::*, types::PyBytes, Bound, PyAny};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct ParametricInstance {
    pub(crate) inner: ommx::ParametricInstance,
    pub(crate) annotations: HashMap<String, String>,
}

crate::annotations::impl_instance_annotations!(
    ParametricInstance,
    "org.ommx.v1.parametric-instance"
);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ParametricInstance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let _guard = crate::TRACING.attach_parent_context(bytes.py());
        let inner = ommx::ParametricInstance::from_bytes(bytes.as_bytes())?;
        Ok(Self {
            inner,
            annotations: HashMap::new(),
        })
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let _guard = crate::TRACING.attach_parent_context(py);
        PyBytes::new(py, &self.inner.to_bytes())
    }

    #[staticmethod]
    #[pyo3(signature = (*, sense, objective, decision_variables, constraints, parameters, named_functions=None, description=None))]
    pub fn from_components(
        sense: Sense,
        objective: Function,
        decision_variables: Vec<DecisionVariable>,
        constraints: BTreeMap<u64, Constraint>,
        parameters: Vec<Parameter>,
        named_functions: Option<Vec<NamedFunction>>,
        description: Option<crate::InstanceDescription>,
    ) -> Result<Self> {
        let mut rust_decision_variables = BTreeMap::new();
        let mut variable_metadata_pairs: Vec<(VariableID, ommx::DecisionVariableMetadata)> =
            Vec::new();
        for var in decision_variables {
            let id = var.0.id();
            variable_metadata_pairs.push((id, var.1.clone()));
            if rust_decision_variables.insert(id, var.0).is_some() {
                anyhow::bail!("Duplicate decision variable ID: {}", id.into_inner());
            }
        }

        let mut constraint_metadata_pairs: Vec<(ConstraintID, ommx::ConstraintMetadata)> =
            Vec::new();
        let rust_constraints: BTreeMap<ConstraintID, ommx::Constraint> = constraints
            .into_iter()
            .map(|(id, c)| {
                let cid = ConstraintID::from(id);
                constraint_metadata_pairs.push((cid, c.1));
                (cid, c.0)
            })
            .collect();

        let mut rust_parameters = BTreeMap::new();
        for p in parameters {
            let id = VariableID::from(p.0.id);
            if rust_parameters.insert(id, p.0).is_some() {
                anyhow::bail!("Duplicate parameter ID: {}", id.into_inner());
            }
        }

        let mut builder = ommx::ParametricInstance::builder()
            .sense(sense.into())
            .objective(objective.0)
            .decision_variables(rust_decision_variables)
            .constraints(rust_constraints)
            .parameters(rust_parameters);

        let mut named_function_metadata_pairs: Vec<(
            ommx::NamedFunctionID,
            ommx::NamedFunctionMetadata,
        )> = Vec::new();
        if let Some(nfs) = named_functions {
            let mut rust_named_functions = BTreeMap::new();
            for nf in nfs {
                let id = nf.0.id;
                named_function_metadata_pairs.push((id, nf.1));
                if rust_named_functions.insert(id, nf.0).is_some() {
                    anyhow::bail!("Duplicate named function ID: {}", id.into_inner());
                }
            }
            builder = builder.named_functions(rust_named_functions);
        }

        if let Some(desc) = description {
            builder = builder.description(desc.0);
        }

        let mut inner = builder.build()?;
        let var_meta = inner.variable_metadata_mut();
        for (id, m) in variable_metadata_pairs {
            var_meta.insert(id, m);
        }
        let constraint_meta = inner.constraint_metadata_mut();
        for (id, m) in constraint_metadata_pairs {
            constraint_meta.insert(id, m);
        }
        let nf_meta = inner.named_function_metadata_mut();
        for (id, m) in named_function_metadata_pairs {
            nf_meta.insert(id, m);
        }

        Ok(Self {
            inner,
            annotations: HashMap::new(),
        })
    }

    /// Create trivial empty instance of minimization with zero objective, no constraints, and no decision variables and parameters.
    #[staticmethod]
    pub fn empty() -> Result<Self> {
        Self::from_components(
            Sense::Minimize,
            Function(ommx::Function::Zero),
            Vec::new(),
            BTreeMap::new(),
            Vec::new(),
            None,
            None,
        )
    }

    /// Substitute parameters to yield an instance.
    ///
    /// Parameters can be provided as a dict mapping parameter IDs to their values.
    pub fn with_parameters(&self, parameters: HashMap<u64, f64>) -> Result<Instance> {
        let mut v1_params = ommx::v1::Parameters::default();
        v1_params.entries = parameters;
        let instance = self.inner.clone().with_parameters(v1_params)?;
        Ok(Instance {
            inner: instance,
            annotations: HashMap::new(),
        })
    }

    #[getter]
    pub fn sense(&self) -> Sense {
        (*self.inner.sense()).into()
    }

    #[getter]
    pub fn objective(&self) -> Function {
        Function(self.inner.objective().clone())
    }

    /// List of all decision variables in the parametric instance sorted by
    /// their IDs (snapshots, suitable for arithmetic).
    #[getter]
    pub fn decision_variables(&self) -> Vec<DecisionVariable> {
        let metadata = self.inner.variable_metadata();
        self.inner
            .decision_variables()
            .iter()
            .map(|(id, var)| DecisionVariable::from_parts(var.clone(), metadata.collect_for(*id)))
            .collect()
    }

    /// Add a decision variable to this parametric instance. Returns an
    /// {class}`~ommx.v1.AttachedDecisionVariable` bound to the variable's
    /// id — a write-through handle for further metadata mutation.
    pub fn add_decision_variable(
        slf: Bound<'_, Self>,
        variable: DecisionVariable,
    ) -> Result<crate::AttachedDecisionVariable> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_decision_variable(variable.0, variable.1)?
        };
        Ok(crate::AttachedDecisionVariable::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    /// Look up the {class}`~ommx.v1.AttachedDecisionVariable` for the given
    /// id — a write-through handle.
    pub fn attached_decision_variable(
        slf: Bound<'_, Self>,
        variable_id: u64,
    ) -> PyResult<crate::AttachedDecisionVariable> {
        let id = VariableID::from(variable_id);
        if !slf.borrow().inner.decision_variables().contains_key(&id) {
            return Err(PyKeyError::new_err(format!(
                "Decision variable with ID {variable_id} not found"
            )));
        }
        Ok(crate::AttachedDecisionVariable::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    /// Dict of all active constraints in the instance keyed by their IDs.
    ///
    /// Each value is an {class}`~ommx.v1.AttachedConstraint`: a write-through
    /// handle whose getters read from this parametric instance's SoA store
    /// and whose metadata setters write back through to it. Use
    /// {meth}`~ommx.v1.AttachedConstraint.detach` to materialize a
    /// {class}`~ommx.v1.Constraint` snapshot if you need an independent copy.
    #[getter]
    pub fn constraints(slf: Bound<'_, Self>) -> BTreeMap<u64, crate::AttachedConstraint> {
        let py = slf.py();
        let ids: Vec<ConstraintID> = slf.borrow().inner.constraints().keys().copied().collect();
        let py_parametric: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                (
                    id.into_inner(),
                    crate::AttachedConstraint::from_parametric(py_parametric.clone_ref(py), id),
                )
            })
            .collect()
    }

    /// Add a regular constraint to this parametric instance.
    ///
    /// Picks an unused {class}`~ommx.v1.ConstraintID`, drains the wrapper's
    /// metadata snapshot into this parametric instance's SoA store, and
    /// returns an {class}`~ommx.v1.AttachedConstraint` bound to the new id.
    /// The input {class}`~ommx.v1.Constraint` is not mutated; subsequent
    /// writes that should land on this parametric instance must go through
    /// the returned handle.
    ///
    /// Raises {class}`ValueError` if the constraint references an id that is
    /// neither a defined decision variable nor a defined parameter, or if it
    /// references an id currently used as a substitution-dependency key.
    pub fn add_constraint(
        slf: Bound<'_, Self>,
        constraint: Constraint,
    ) -> Result<crate::AttachedConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedConstraint::from_parametric(slf.unbind(), id))
    }

    #[getter]
    pub fn removed_constraints(&self) -> BTreeMap<u64, RemovedConstraint> {
        let metadata = self.inner.constraint_collection().metadata();
        self.inner
            .removed_constraints()
            .iter()
            .map(|(id, (c, r))| {
                (
                    id.into_inner(),
                    RemovedConstraint::from_parts(c.clone(), metadata.collect_for(*id), r.clone()),
                )
            })
            .collect()
    }

    /// Dict of all active indicator constraints in the parametric instance
    /// keyed by their IDs.
    ///
    /// Each value is an {class}`~ommx.v1.AttachedIndicatorConstraint`: a
    /// write-through handle whose getters read from this parametric
    /// instance's SoA store and whose metadata setters write back through
    /// to it.
    #[getter]
    pub fn indicator_constraints(
        slf: Bound<'_, Self>,
    ) -> BTreeMap<u64, crate::AttachedIndicatorConstraint> {
        let py = slf.py();
        let ids: Vec<ommx::IndicatorConstraintID> = slf
            .borrow()
            .inner
            .indicator_constraints()
            .keys()
            .copied()
            .collect();
        let py_parametric: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                (
                    id.into_inner(),
                    crate::AttachedIndicatorConstraint::from_parametric(
                        py_parametric.clone_ref(py),
                        id,
                    ),
                )
            })
            .collect()
    }

    /// Add an indicator constraint to this parametric instance.
    ///
    /// Picks an unused {class}`~ommx.v1.IndicatorConstraintID`, drains the
    /// wrapper's metadata snapshot into this parametric instance's SoA
    /// store, and returns an
    /// {class}`~ommx.v1.AttachedIndicatorConstraint` bound to the new id.
    ///
    /// Raises {class}`ValueError` if the constraint references an id that
    /// is neither a defined decision variable nor a defined parameter, or
    /// if it references an id currently used as a substitution-dependency
    /// key.
    pub fn add_indicator_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::IndicatorConstraint,
    ) -> Result<crate::AttachedIndicatorConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_indicator_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedIndicatorConstraint::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    /// Dict of all active one-hot constraints in the parametric instance
    /// keyed by their IDs. Each value is an
    /// {class}`~ommx.v1.AttachedOneHotConstraint`.
    #[getter]
    pub fn one_hot_constraints(
        slf: Bound<'_, Self>,
    ) -> BTreeMap<u64, crate::AttachedOneHotConstraint> {
        let py = slf.py();
        let ids: Vec<ommx::OneHotConstraintID> = slf
            .borrow()
            .inner
            .one_hot_constraints()
            .keys()
            .copied()
            .collect();
        let py_parametric: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                (
                    id.into_inner(),
                    crate::AttachedOneHotConstraint::from_parametric(
                        py_parametric.clone_ref(py),
                        id,
                    ),
                )
            })
            .collect()
    }

    /// Add a one-hot constraint to this parametric instance.
    pub fn add_one_hot_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::OneHotConstraint,
    ) -> Result<crate::AttachedOneHotConstraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner
                .add_one_hot_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedOneHotConstraint::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    /// Dict of all active SOS1 constraints in the parametric instance keyed
    /// by their IDs. Each value is an {class}`~ommx.v1.AttachedSos1Constraint`.
    #[getter]
    pub fn sos1_constraints(slf: Bound<'_, Self>) -> BTreeMap<u64, crate::AttachedSos1Constraint> {
        let py = slf.py();
        let ids: Vec<ommx::Sos1ConstraintID> = slf
            .borrow()
            .inner
            .sos1_constraints()
            .keys()
            .copied()
            .collect();
        let py_parametric: Py<Self> = slf.unbind();
        ids.into_iter()
            .map(|id| {
                (
                    id.into_inner(),
                    crate::AttachedSos1Constraint::from_parametric(py_parametric.clone_ref(py), id),
                )
            })
            .collect()
    }

    /// Add a SOS1 constraint to this parametric instance.
    pub fn add_sos1_constraint(
        slf: Bound<'_, Self>,
        constraint: crate::Sos1Constraint,
    ) -> Result<crate::AttachedSos1Constraint> {
        let id = {
            let mut inst = slf.borrow_mut();
            inst.inner.add_sos1_constraint(constraint.0, constraint.1)?
        };
        Ok(crate::AttachedSos1Constraint::from_parametric(
            slf.unbind(),
            id,
        ))
    }

    #[getter]
    pub fn named_functions(&self) -> Vec<NamedFunction> {
        let metadata = self.inner.named_function_metadata();
        self.inner
            .named_functions()
            .iter()
            .map(|(id, nf)| NamedFunction(nf.clone(), metadata.collect_for(*id)))
            .collect()
    }

    #[getter]
    pub fn parameters(&self) -> Vec<Parameter> {
        self.inner
            .parameters()
            .values()
            .map(|p| Parameter(p.clone()))
            .collect()
    }

    #[getter]
    pub fn description(&self) -> Option<crate::InstanceDescription> {
        self.inner
            .description
            .as_ref()
            .map(|desc| crate::InstanceDescription(desc.clone()))
    }

    #[getter]
    pub fn decision_variable_ids(&self) -> BTreeSet<u64> {
        self.inner
            .decision_variables()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    #[getter]
    pub fn parameter_ids(&self) -> BTreeSet<u64> {
        self.inner
            .parameters()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Get a specific decision variable by ID
    pub fn get_decision_variable_by_id(&self, variable_id: u64) -> PyResult<DecisionVariable> {
        let var_id = VariableID::from(variable_id);
        let metadata = self.inner.variable_metadata();
        self.inner
            .decision_variables()
            .get(&var_id)
            .map(|var| DecisionVariable::from_parts(var.clone(), metadata.collect_for(var_id)))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Decision variable with ID {variable_id} not found"))
            })
    }

    /// Get a specific constraint by ID
    pub fn get_constraint_by_id(&self, constraint_id: u64) -> PyResult<Constraint> {
        let cid = ConstraintID::from(constraint_id);
        let metadata = self.inner.constraint_collection().metadata();
        self.inner
            .constraints()
            .get(&cid)
            .map(|c| Constraint::from_parts(c.clone(), metadata.collect_for(cid)))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Constraint with ID {constraint_id} not found"))
            })
    }

    /// Get a specific removed constraint by ID
    pub fn get_removed_constraint_by_id(&self, constraint_id: u64) -> PyResult<RemovedConstraint> {
        let cid = ConstraintID::from(constraint_id);
        let metadata = self.inner.constraint_collection().metadata();
        self.inner
            .removed_constraints()
            .get(&cid)
            .map(|(c, r)| {
                RemovedConstraint::from_parts(c.clone(), metadata.collect_for(cid), r.clone())
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Removed constraint with ID {constraint_id} not found"
                ))
            })
    }

    /// Get a specific named function by ID
    pub fn get_named_function_by_id(&self, named_function_id: u64) -> PyResult<NamedFunction> {
        let id = NamedFunctionID::from(named_function_id);
        self.inner
            .named_functions()
            .get(&id)
            .map(|nf| {
                NamedFunction(
                    nf.clone(),
                    self.inner.named_function_metadata().collect_for(id),
                )
            })
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "Named function with ID {named_function_id} not found"
                ))
            })
    }

    /// Get a specific parameter by ID
    pub fn get_parameter_by_id(&self, parameter_id: u64) -> PyResult<Parameter> {
        self.inner
            .parameters()
            .get(&VariableID::from(parameter_id))
            .map(|p| Parameter(p.clone()))
            .ok_or_else(|| {
                PyKeyError::new_err(format!("Parameter with ID {parameter_id} not found"))
            })
    }

    /// DataFrame of decision variables
    #[pyo3(signature = (include = None))]
    pub fn decision_variables_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let var_meta_store = self.inner.variable_metadata().clone();
        let view: Vec<(ommx::DecisionVariableMetadata, &ommx::DecisionVariable)> = self
            .inner
            .decision_variables()
            .iter()
            .map(|(id, dv)| (var_meta_store.collect_for(*id), dv))
            .collect();
        entries_to_dataframe(
            py,
            view.iter()
                .map(|(m, dv)| crate::pandas::WithMetadata::new(*dv, m)),
            "id",
            flags,
        )
    }

    /// DataFrame of constraints, dispatched on `kind=`. See
    /// {meth}`ommx.v1.Instance.constraints_df` for column / `kind=` /
    /// `include=` / `removed=` semantics.
    #[pyo3(signature = (kind = ConstraintKind::Regular, include = None, removed = false))]
    pub fn constraints_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
        include: Option<Vec<String>>,
        removed: bool,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        let mut flags = crate::pandas::IncludeFlags::from_optional(include)?;
        if removed {
            flags.removed_reason = true;
        }
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                let meta = coll.metadata().clone();
                let active = coll.active();
                let removed_map = coll.removed();
                let mut entries: Vec<Bound<'py, pyo3::types::PyAny>> = Vec::new();
                if removed {
                    let mut ai = active.iter().peekable();
                    let mut ri = removed_map.iter().peekable();
                    loop {
                        let pick_active = match (ai.peek(), ri.peek()) {
                            (Some((aid, _)), Some((rid, _))) => aid <= rid,
                            (Some(_), None) => true,
                            (None, Some(_)) => false,
                            (None, None) => break,
                        };
                        if pick_active {
                            let (id, c) = ai.next().unwrap();
                            let m = meta.collect_for(*id);
                            let dict = crate::pandas::WithMetadata::new((*id, c), &m)
                                .to_pandas_entry(py)?;
                            crate::pandas::set_removed_reason_na(&dict)?;
                            crate::pandas::apply_include_filter(&dict, flags)?;
                            crate::pandas::rename_id_column(&dict, id_col)?;
                            entries.push(dict.into_any());
                        } else {
                            let (id, pair) = ri.next().unwrap();
                            let m = meta.collect_for(*id);
                            let dict = crate::pandas::WithMetadata::new((*id, pair), &m)
                                .to_pandas_entry(py)?;
                            crate::pandas::apply_include_filter(&dict, flags)?;
                            crate::pandas::rename_id_column(&dict, id_col)?;
                            entries.push(dict.into_any());
                        }
                    }
                } else {
                    for (id, c) in active.iter() {
                        let m = meta.collect_for(*id);
                        let dict =
                            crate::pandas::WithMetadata::new((*id, c), &m).to_pandas_entry(py)?;
                        if flags.removed_reason {
                            crate::pandas::set_removed_reason_na(&dict)?;
                        }
                        crate::pandas::apply_include_filter(&dict, flags)?;
                        crate::pandas::rename_id_column(&dict, id_col)?;
                        entries.push(dict.into_any());
                    }
                }
                crate::pandas::raw_entries_to_dataframe(py, entries, id_col)
            }
        )
    }

    /// DataFrame of named functions
    #[pyo3(signature = (include = None))]
    pub fn named_functions_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        let nf_meta_store = self.inner.named_function_metadata().clone();
        let nf_meta_view: Vec<(ommx::NamedFunctionMetadata, &ommx::NamedFunction)> = self
            .inner
            .named_functions()
            .iter()
            .map(|(id, nf)| (nf_meta_store.collect_for(*id), nf))
            .collect();
        entries_to_dataframe(
            py,
            nf_meta_view
                .iter()
                .map(|(m, nf)| crate::pandas::WithMetadata::new(*nf, m)),
            "id",
            flags,
        )
    }

    /// DataFrame of parameters
    #[pyo3(signature = (include = None))]
    pub fn parameters_df<'py>(
        &self,
        py: Python<'py>,
        include: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let flags = crate::pandas::IncludeFlags::from_optional(include)?;
        entries_to_dataframe(py, self.inner.parameters().values(), "id", flags)
    }

    /// Constraint metadata DataFrame (id-indexed). See
    /// {meth}`ommx.v1.Instance.constraint_metadata_df` for column / `kind=`
    /// semantics.
    #[pyo3(signature = (kind = ConstraintKind::Regular))]
    pub fn constraint_metadata_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_metadata_dataframe(
                    py,
                    coll.metadata(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Constraint parameters DataFrame (long format).
    #[pyo3(signature = (kind = ConstraintKind::Regular))]
    pub fn constraint_parameters_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_parameters_dataframe(
                    py,
                    coll.metadata(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Constraint provenance DataFrame (long format).
    #[pyo3(signature = (kind = ConstraintKind::Regular))]
    pub fn constraint_provenance_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_provenance_dataframe(
                    py,
                    coll.metadata(),
                    coll.active().keys().chain(coll.removed().keys()).copied(),
                    id_col,
                )
            }
        )
    }

    /// Removed-constraint reasons DataFrame (long format).
    #[pyo3(signature = (kind = ConstraintKind::Regular))]
    pub fn constraint_removed_reasons_df<'py>(
        &self,
        py: Python<'py>,
        kind: ConstraintKind,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let id_col = constraint_id_col(kind);
        constraint_kind_collection!(
            self.inner,
            kind,
            [
                constraint_collection,
                indicator_constraint_collection,
                one_hot_constraint_collection,
                sos1_constraint_collection
            ],
            |coll| {
                crate::pandas::constraint_removed_reasons_dataframe(
                    py,
                    coll.removed().iter().map(|(id, (_, r))| (*id, r)),
                    id_col,
                )
            }
        )
    }

    /// Decision-variable metadata DataFrame (id-indexed).
    pub fn variable_metadata_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDataFrame>> {
        crate::pandas::variable_metadata_dataframe(
            py,
            self.inner.variable_metadata(),
            self.inner.decision_variables().keys().copied(),
            "variable_id",
        )
    }

    /// Decision-variable parameters DataFrame (long format).
    pub fn variable_parameters_df<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        crate::pandas::variable_parameters_dataframe(
            py,
            self.inner.variable_metadata(),
            self.inner.decision_variables().keys().copied(),
            "variable_id",
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
