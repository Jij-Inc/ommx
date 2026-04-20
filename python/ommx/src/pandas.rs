//! Thin wrapper around `pandas.DataFrame` for type-safe PyO3 bindings,
//! plus shared helpers for building DataFrames from domain objects.

use fnv::FnvHashMap;
use ommx::{Evaluate, VariableIDSet};
use pyo3::{
    prelude::*,
    sync::PyOnceLock,
    types::{PyAny, PyDict, PyList, PySet, PyType},
    Bound, Py, PyTypeCheck,
};

// ---------------------------------------------------------------------------
// PyDataFrame wrapper
// ---------------------------------------------------------------------------

/// A transparent wrapper around `pandas.DataFrame`.
///
/// This is not a `#[pyclass]` — it wraps an existing Python type
/// similar to how PyO3 wraps `datetime.datetime` as `PyDateTime`.
#[repr(transparent)]
pub struct PyDataFrame(PyAny);

static DATAFRAME_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();
static PANDAS_NA: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

fn get_dataframe_type(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
    Ok(DATAFRAME_TYPE
        .get_or_try_init(py, || -> PyResult<Py<PyType>> {
            let pandas = py.import("pandas")?;
            let df_type = pandas.getattr("DataFrame")?;
            Ok(df_type.cast_into::<PyType>()?.unbind())
        })?
        .bind(py))
}

unsafe impl PyTypeCheck for PyDataFrame {
    const NAME: &'static str = "DataFrame";

    fn type_check(object: &Bound<'_, PyAny>) -> bool {
        let py = object.py();
        match get_dataframe_type(py) {
            Ok(t) => object.is_instance(t).unwrap_or(false),
            Err(_) => false,
        }
    }

    fn classinfo_object(py: Python<'_>) -> Bound<'_, PyAny> {
        get_dataframe_type(py)
            .expect("Failed to import pandas.DataFrame")
            .clone()
            .into_any()
    }
}

impl pyo3_stub_gen::PyStubType for PyDataFrame {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: "pandas.DataFrame".to_string(),
            source_module: None,
            import: ["pandas".into()].into(),
            type_refs: Default::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// pandas.NA cache
// ---------------------------------------------------------------------------

/// Get `pandas.NA`, cached in a static `PyOnceLock`.
pub fn get_na<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PANDAS_NA
        .get_or_try_init(py, || -> PyResult<Py<PyAny>> {
            let pandas = py.import("pandas")?;
            Ok(pandas.getattr("NA")?.unbind())
        })?
        .bind(py)
        .clone())
}

// ---------------------------------------------------------------------------
// Trait: ToPandasEntry
// ---------------------------------------------------------------------------

/// Trait for converting a domain object into a Python dict suitable for `pd.DataFrame([...])`.
pub trait ToPandasEntry {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>>;
}

// ---------------------------------------------------------------------------
// Helper: entries_to_dataframe
// ---------------------------------------------------------------------------

/// Blanket impl so that `&T` also implements `ToPandasEntry`.
impl<T: ToPandasEntry> ToPandasEntry for &T {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        (*self).to_pandas_entry(py)
    }
}

/// Build a `pandas.DataFrame` from an iterator of domain objects, indexed by `index_col`.
pub fn entries_to_dataframe<'py, T: ToPandasEntry>(
    py: Python<'py>,
    items: impl Iterator<Item = T>,
    index_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>> {
    let entries: Vec<Bound<'py, PyAny>> = items
        .map(|item| item.to_pandas_entry(py).map(|d| d.into_any()))
        .collect::<PyResult<_>>()?;
    raw_entries_to_dataframe(py, entries, index_col)
}

/// Build a `pandas.DataFrame` from pre-built entry dicts, indexed by `index_col`.
///
/// Use this when entries need custom logic (e.g. SampleSet dynamic columns).
pub fn raw_entries_to_dataframe<'py>(
    py: Python<'py>,
    entries: Vec<Bound<'py, PyAny>>,
    index_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>> {
    let pandas = py.import("pandas")?;
    let df = pandas.call_method1("DataFrame", (entries,))?;
    if df.getattr("empty")?.extract::<bool>()? {
        return df.cast_into().map_err(Into::into);
    }
    df.call_method1("set_index", (index_col,))?
        .cast_into()
        .map_err(Into::into)
}

/// Build a sorted `pandas.DataFrame` from pre-built entry dicts.
///
/// Sorts by `sort_by` columns before setting the index.
pub fn sorted_entries_to_dataframe<'py>(
    py: Python<'py>,
    entries: Vec<Bound<'py, PyAny>>,
    sort_by: &[&str],
    ascending: &[bool],
    index_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>> {
    let pandas = py.import("pandas")?;
    let df = pandas.call_method1("DataFrame", (entries,))?;
    if df.getattr("empty")?.extract::<bool>()? {
        return df.cast_into().map_err(Into::into);
    }
    let sort_args = PyDict::new(py);
    sort_args.set_item("by", sort_by.to_vec())?;
    sort_args.set_item("ascending", ascending.to_vec())?;
    let sorted = df.call_method("sort_values", (), Some(&sort_args))?;
    sorted
        .call_method1("set_index", (index_col,))?
        .cast_into()
        .map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Helpers: metadata fields
// ---------------------------------------------------------------------------

/// Set common metadata fields (name, subscripts, description) on a PyDict.
///
/// Uses cached `pandas.NA` for missing optional values.
pub fn set_metadata<'py>(
    dict: &Bound<'py, PyDict>,
    name: Option<&str>,
    subscripts: &[i64],
    description: Option<&str>,
) -> PyResult<()> {
    let py = dict.py();
    let na = get_na(py)?;
    match name.filter(|n| !n.is_empty()) {
        Some(n) => dict.set_item("name", n)?,
        None => dict.set_item("name", &na)?,
    }
    dict.set_item("subscripts", PyList::new(py, subscripts.iter())?)?;
    match description.filter(|d| !d.is_empty()) {
        Some(d) => dict.set_item("description", d)?,
        None => dict.set_item("description", &na)?,
    }
    Ok(())
}

/// Set `parameters.{key}` columns from a string-string map.
pub fn set_parameter_columns(
    dict: &Bound<PyDict>,
    parameters: &FnvHashMap<String, String>,
) -> PyResult<()> {
    for (key, value) in parameters {
        dict.set_item(format!("parameters.{key}"), value)?;
    }
    Ok(())
}

/// Set `used_ids` column from a `VariableIDSet` as a Python set.
pub fn set_used_ids<'py>(dict: &Bound<'py, PyDict>, ids: &VariableIDSet) -> PyResult<()> {
    let id_vec: Vec<u64> = ids.iter().map(|id| id.into_inner()).collect();
    dict.set_item("used_ids", PySet::new(dict.py(), &id_vec)?)?;
    Ok(())
}

/// Set equality column as a string.
pub fn set_equality(dict: &Bound<PyDict>, equality: ommx::Equality) -> PyResult<()> {
    let s = match equality {
        ommx::Equality::EqualToZero => "=0",
        ommx::Equality::LessThanOrEqualToZero => "<=0",
    };
    dict.set_item("equality", s)
}

/// Set kind column as a string.
pub fn set_kind(dict: &Bound<PyDict>, kind: ommx::Kind) -> PyResult<()> {
    let s = match kind {
        ommx::Kind::Binary => "Binary",
        ommx::Kind::Integer => "Integer",
        ommx::Kind::Continuous => "Continuous",
        ommx::Kind::SemiInteger => "SemiInteger",
        ommx::Kind::SemiContinuous => "SemiContinuous",
    };
    dict.set_item("kind", s)
}

/// Set function type column as a string.
pub fn set_function_type(dict: &Bound<PyDict>, function: &ommx::Function) -> PyResult<()> {
    let s = match function {
        ommx::Function::Zero => "Zero",
        ommx::Function::Constant(_) => "Constant",
        ommx::Function::Linear(_) => "Linear",
        ommx::Function::Quadratic(_) => "Quadratic",
        ommx::Function::Polynomial(_) => "Polynomial",
    };
    dict.set_item("type", s)
}

// ---------------------------------------------------------------------------
// ToPandasEntry implementations for unevaluated types
// ---------------------------------------------------------------------------

impl ToPandasEntry for ommx::DecisionVariable {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", self.id().into_inner())?;
        set_kind(&dict, self.kind())?;
        dict.set_item("lower", self.bound().lower())?;
        dict.set_item("upper", self.bound().upper())?;
        set_metadata(
            &dict,
            self.metadata.name.as_deref(),
            &self.metadata.subscripts,
            self.metadata.description.as_deref(),
        )?;
        match self.substituted_value() {
            Some(v) => dict.set_item("substituted_value", v)?,
            None => dict.set_item("substituted_value", &na)?,
        }
        set_parameter_columns(&dict, &self.metadata.parameters)?;
        Ok(dict)
    }
}

impl ToPandasEntry for (ommx::ConstraintID, &ommx::Constraint) {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_equality(&dict, c.equality)?;
        set_function_type(&dict, &c.stage.function)?;
        set_used_ids(&dict, &c.stage.function.required_ids())?;
        set_metadata(
            &dict,
            c.metadata.name.as_deref(),
            &c.metadata.subscripts,
            c.metadata.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl ToPandasEntry for (ommx::IndicatorConstraintID, &ommx::IndicatorConstraint) {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        dict.set_item("indicator_variable_id", c.indicator_variable.into_inner())?;
        set_equality(&dict, c.equality)?;
        set_function_type(&dict, &c.stage.function)?;
        // Include indicator_variable in used_ids
        let mut used_ids = c.stage.function.required_ids();
        used_ids.insert(c.indicator_variable);
        set_used_ids(&dict, &used_ids)?;
        set_metadata(
            &dict,
            c.metadata.name.as_deref(),
            &c.metadata.subscripts,
            c.metadata.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl ToPandasEntry
    for (
        ommx::IndicatorConstraintID,
        &ommx::EvaluatedIndicatorConstraint,
    )
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        dict.set_item("indicator_variable_id", c.indicator_variable.into_inner())?;
        set_equality(&dict, c.equality)?;
        dict.set_item("value", c.stage.evaluated_value)?;
        dict.set_item("indicator_active", c.stage.indicator_active)?;
        set_used_ids(&dict, &c.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            c.metadata.name.as_deref(),
            &c.metadata.subscripts,
            c.metadata.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl<'a> ToPandasEntry
    for WithSampleIds<
        'a,
        (
            ommx::IndicatorConstraintID,
            &'a ommx::SampledIndicatorConstraint,
        ),
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, ic) = &self.item;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        dict.set_item("indicator_variable_id", ic.indicator_variable.into_inner())?;
        set_equality(&dict, ic.equality)?;
        set_used_ids(&dict, &ic.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            ic.metadata.name.as_deref(),
            &ic.metadata.subscripts,
            ic.metadata.description.as_deref(),
        )?;
        for &sample_id in self.sample_ids {
            let value = ic.stage.evaluated_values.get(sample_id).ok().copied();
            dict.set_item(format!("value.{}", sample_id.into_inner()), value)?;
            let feas = ic.stage.feasible.get(&sample_id).copied();
            dict.set_item(format!("feasible.{}", sample_id.into_inner()), feas)?;
            let active = ic.stage.indicator_active.get(&sample_id).copied();
            dict.set_item(
                format!("indicator_active.{}", sample_id.into_inner()),
                active,
            )?;
        }
        Ok(dict)
    }
}

impl ToPandasEntry
    for (
        ommx::IndicatorConstraintID,
        &(ommx::IndicatorConstraint, ommx::RemovedReason),
    )
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, inner) = self;
        let (ic, reason) = inner;
        let dict = (*id, ic).to_pandas_entry(py)?;
        dict.set_item("removed_reason", &reason.reason)?;
        for (key, value) in &reason.parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict)
    }
}

impl ToPandasEntry for (ommx::OneHotConstraintID, &ommx::OneHotConstraint) {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, one_hot) = self;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        let vars: Vec<u64> = one_hot.variables.iter().map(|v| v.into_inner()).collect();
        dict.set_item("variables", PySet::new(py, &vars)?)?;
        dict.set_item("num_variables", vars.len())?;
        set_used_ids(&dict, &one_hot.variables)?;
        set_metadata(
            &dict,
            one_hot.metadata.name.as_deref(),
            &one_hot.metadata.subscripts,
            one_hot.metadata.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl ToPandasEntry
    for (
        ommx::OneHotConstraintID,
        &(ommx::OneHotConstraint, ommx::RemovedReason),
    )
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, inner) = self;
        let (one_hot, reason) = inner;
        let dict = (*id, one_hot).to_pandas_entry(py)?;
        dict.set_item("removed_reason", &reason.reason)?;
        for (key, value) in &reason.parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict)
    }
}

impl ToPandasEntry for (ommx::Sos1ConstraintID, &ommx::Sos1Constraint) {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, sos1) = self;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        let vars: Vec<u64> = sos1.variables.iter().map(|v| v.into_inner()).collect();
        dict.set_item("variables", PySet::new(py, &vars)?)?;
        dict.set_item("num_variables", vars.len())?;
        set_used_ids(&dict, &sos1.variables)?;
        set_metadata(
            &dict,
            sos1.metadata.name.as_deref(),
            &sos1.metadata.subscripts,
            sos1.metadata.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl ToPandasEntry
    for (
        ommx::Sos1ConstraintID,
        &(ommx::Sos1Constraint, ommx::RemovedReason),
    )
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, inner) = self;
        let (sos1, reason) = inner;
        let dict = (*id, sos1).to_pandas_entry(py)?;
        dict.set_item("removed_reason", &reason.reason)?;
        for (key, value) in &reason.parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict)
    }
}

impl ToPandasEntry for (ommx::ConstraintID, &(ommx::Constraint, ommx::RemovedReason)) {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, inner) = self;
        let (constraint, reason) = inner;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_equality(&dict, constraint.equality)?;
        set_function_type(&dict, &constraint.stage.function)?;
        set_used_ids(&dict, &constraint.stage.function.required_ids())?;
        set_metadata(
            &dict,
            constraint.metadata.name.as_deref(),
            &constraint.metadata.subscripts,
            constraint.metadata.description.as_deref(),
        )?;
        dict.set_item("removed_reason", &reason.reason)?;
        for (key, value) in &reason.parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict)
    }
}

impl ToPandasEntry for ommx::NamedFunction {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.id.into_inner())?;
        set_function_type(&dict, &self.function)?;
        dict.set_item("function", crate::Function(self.function.clone()))?;
        set_used_ids(&dict, &self.function.required_ids())?;
        set_metadata(
            &dict,
            self.name.as_deref(),
            &self.subscripts,
            self.description.as_deref(),
        )?;
        set_parameter_columns(&dict, &self.parameters)?;
        Ok(dict)
    }
}

impl ToPandasEntry for ommx::v1::Parameter {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.id)?;
        set_metadata(
            &dict,
            self.name.as_deref(),
            &self.subscripts,
            self.description.as_deref(),
        )?;
        for (key, value) in &self.parameters {
            dict.set_item(format!("parameters.{key}"), value)?;
        }
        Ok(dict)
    }
}

// ---------------------------------------------------------------------------
// ToPandasEntry implementations for evaluated types (Solution)
// ---------------------------------------------------------------------------

impl ToPandasEntry for ommx::EvaluatedDecisionVariable {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", self.id().into_inner())?;
        set_kind(&dict, *self.kind())?;
        dict.set_item("lower", self.bound().lower())?;
        dict.set_item("upper", self.bound().upper())?;
        set_metadata(
            &dict,
            self.metadata.name.as_deref(),
            &self.metadata.subscripts,
            self.metadata.description.as_deref(),
        )?;
        // EvaluatedDecisionVariable has no substituted_value field
        dict.set_item("substituted_value", &na)?;
        dict.set_item("value", *self.value())?;
        Ok(dict)
    }
}

impl ToPandasEntry for (ommx::ConstraintID, &ommx::EvaluatedConstraint) {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self;
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_equality(&dict, c.equality)?;
        dict.set_item("value", c.stage.evaluated_value)?;
        set_used_ids(&dict, &c.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            c.metadata.name.as_deref(),
            &c.metadata.subscripts,
            c.metadata.description.as_deref(),
        )?;
        match c.stage.dual_variable {
            Some(v) => dict.set_item("dual_variable", v)?,
            None => dict.set_item("dual_variable", &na)?,
        }
        Ok(dict)
    }
}

/// Entry for removed_reasons_df: constraint_id → removed_reason, removed_reason.{key}
pub struct RemovedReasonEntry<'a> {
    pub id: u64,
    pub reason: &'a ommx::RemovedReason,
}

impl ToPandasEntry for RemovedReasonEntry<'_> {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.id)?;
        dict.set_item("removed_reason", &self.reason.reason)?;
        for (key, value) in &self.reason.parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict)
    }
}

impl ToPandasEntry for ommx::EvaluatedNamedFunction {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.id.into_inner())?;
        dict.set_item("value", self.evaluated_value())?;
        set_used_ids(&dict, self.used_decision_variable_ids())?;
        set_metadata(
            &dict,
            self.name.as_deref(),
            &self.subscripts,
            self.description.as_deref(),
        )?;
        set_parameter_columns(&dict, &self.parameters)?;
        Ok(dict)
    }
}

// ---------------------------------------------------------------------------
// ToPandasEntry implementations for sampled types (SampleSet)
// ---------------------------------------------------------------------------

/// Wrapper that pairs a sampled item with the global sorted sample IDs,
/// so `ToPandasEntry` can generate per-sample dynamic columns.
pub struct WithSampleIds<'a, T> {
    pub item: T,
    pub sample_ids: &'a [ommx::SampleID],
}

impl<'a> ToPandasEntry for WithSampleIds<'a, &'a ommx::SampledDecisionVariable> {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dv = self.item;
        let dict = PyDict::new(py);
        dict.set_item("id", dv.id().into_inner())?;
        set_kind(&dict, *dv.kind())?;
        dict.set_item("lower", dv.bound().lower())?;
        dict.set_item("upper", dv.bound().upper())?;
        set_metadata(
            &dict,
            dv.metadata.name.as_deref(),
            &dv.metadata.subscripts,
            dv.metadata.description.as_deref(),
        )?;
        for &sample_id in self.sample_ids {
            let value = dv.samples().get(sample_id).ok().copied();
            dict.set_item(sample_id.into_inner(), value)?;
        }
        Ok(dict)
    }
}

impl<'a> ToPandasEntry for WithSampleIds<'a, (ommx::ConstraintID, &'a ommx::SampledConstraint)> {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, sc) = &self.item;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_equality(&dict, sc.equality)?;
        set_used_ids(&dict, &sc.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            sc.metadata.name.as_deref(),
            &sc.metadata.subscripts,
            sc.metadata.description.as_deref(),
        )?;
        for &sample_id in self.sample_ids {
            let value = sc.stage.evaluated_values.get(sample_id).ok().copied();
            dict.set_item(format!("value.{}", sample_id.into_inner()), value)?;
            let feas = sc.stage.feasible.get(&sample_id).copied();
            dict.set_item(format!("feasible.{}", sample_id.into_inner()), feas)?;
        }
        Ok(dict)
    }
}

impl<'a> ToPandasEntry for WithSampleIds<'a, &'a ommx::SampledNamedFunction> {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let nf = self.item;
        let dict = PyDict::new(py);
        dict.set_item("id", nf.id().into_inner())?;
        set_used_ids(&dict, nf.used_decision_variable_ids())?;
        set_metadata(
            &dict,
            nf.name.as_deref(),
            &nf.subscripts,
            nf.description.as_deref(),
        )?;
        let params: Vec<String> = nf
            .parameters
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        dict.set_item("parameters", params)?;
        for &sample_id in self.sample_ids {
            let value = nf.evaluated_values().get(sample_id).ok().copied();
            dict.set_item(sample_id.into_inner(), value)?;
        }
        Ok(dict)
    }
}
