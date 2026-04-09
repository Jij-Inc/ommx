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
// Trait: ToPandasEntry
// ---------------------------------------------------------------------------

/// Trait for converting a domain object into a Python dict suitable for `pd.DataFrame([...])`.
pub trait ToPandasEntry {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>>;
}

// ---------------------------------------------------------------------------
// Helper: entries_to_dataframe
// ---------------------------------------------------------------------------

/// Blanket impl so that `&T` also implements `ToPandasEntry`.
impl<T: ToPandasEntry> ToPandasEntry for &T {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        (*self).to_pandas_entry(py, na)
    }
}

/// Build a `pandas.DataFrame` from an iterator of domain objects, indexed by `index_col`.
pub fn entries_to_dataframe<'py, T: ToPandasEntry>(
    py: Python<'py>,
    items: impl Iterator<Item = T>,
    index_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>> {
    let pandas = py.import("pandas")?;
    let na = pandas.getattr("NA")?;
    let entries: Vec<_> = items
        .map(|item| item.to_pandas_entry(py, &na))
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
/// Uses `pandas.NA` (the `na` argument) for missing optional values.
pub fn set_metadata<'py>(
    dict: &Bound<'py, PyDict>,
    name: Option<&str>,
    subscripts: &[i64],
    description: Option<&str>,
    na: &Bound<'py, PyAny>,
) -> PyResult<()> {
    match name.filter(|n| !n.is_empty()) {
        Some(n) => dict.set_item("name", n)?,
        None => dict.set_item("name", na)?,
    }
    dict.set_item("subscripts", PyList::new(dict.py(), subscripts.iter())?)?;
    match description.filter(|d| !d.is_empty()) {
        Some(d) => dict.set_item("description", d)?,
        None => dict.set_item("description", na)?,
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
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
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
            na,
        )?;
        match self.substituted_value() {
            Some(v) => dict.set_item("substituted_value", v)?,
            None => dict.set_item("substituted_value", na)?,
        }
        set_parameter_columns(&dict, &self.metadata.parameters)?;
        Ok(dict.into_any())
    }
}

impl ToPandasEntry for ommx::Constraint {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.id.into_inner())?;
        set_equality(&dict, self.equality)?;
        set_function_type(&dict, &self.function)?;
        set_used_ids(&dict, &self.function.required_ids())?;
        set_metadata(
            &dict,
            self.name.as_deref(),
            &self.subscripts,
            self.description.as_deref(),
            na,
        )?;
        let _ = na; // suppress unused warning
        Ok(dict.into_any())
    }
}

impl ToPandasEntry for ommx::RemovedConstraint {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.constraint.id.into_inner())?;
        set_equality(&dict, self.constraint.equality)?;
        set_function_type(&dict, &self.constraint.function)?;
        set_used_ids(&dict, &self.constraint.function.required_ids())?;
        set_metadata(
            &dict,
            self.constraint.name.as_deref(),
            &self.constraint.subscripts,
            self.constraint.description.as_deref(),
            na,
        )?;
        dict.set_item("removed_reason", &self.removed_reason)?;
        for (key, value) in &self.removed_reason_parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict.into_any())
    }
}

impl ToPandasEntry for ommx::NamedFunction {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
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
            na,
        )?;
        set_parameter_columns(&dict, &self.parameters)?;
        Ok(dict.into_any())
    }
}

impl ToPandasEntry for ommx::v1::Parameter {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.id)?;
        set_metadata(
            &dict,
            self.name.as_deref(),
            &self.subscripts,
            self.description.as_deref(),
            na,
        )?;
        for (key, value) in &self.parameters {
            dict.set_item(format!("parameters.{key}"), value)?;
        }
        Ok(dict.into_any())
    }
}

// ---------------------------------------------------------------------------
// ToPandasEntry implementations for evaluated types (Solution)
// ---------------------------------------------------------------------------

impl ToPandasEntry for ommx::EvaluatedDecisionVariable {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
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
            na,
        )?;
        // EvaluatedDecisionVariable has no substituted_value field
        dict.set_item("substituted_value", na)?;
        dict.set_item("value", *self.value())?;
        Ok(dict.into_any())
    }
}

impl ToPandasEntry for ommx::EvaluatedConstraint {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.id().into_inner())?;
        set_equality(&dict, *self.equality())?;
        dict.set_item("value", *self.evaluated_value())?;
        set_used_ids(&dict, self.used_decision_variable_ids())?;
        set_metadata(
            &dict,
            self.metadata.name.as_deref(),
            &self.metadata.subscripts,
            self.metadata.description.as_deref(),
            na,
        )?;
        match self.dual_variable {
            Some(v) => dict.set_item("dual_variable", v)?,
            None => dict.set_item("dual_variable", na)?,
        }
        match self.removed_reason() {
            Some(r) => dict.set_item("removed_reason", r)?,
            None => dict.set_item("removed_reason", na)?,
        }
        Ok(dict.into_any())
    }
}

impl ToPandasEntry for ommx::EvaluatedNamedFunction {
    fn to_pandas_entry<'py>(
        &self,
        py: Python<'py>,
        na: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let dict = PyDict::new(py);
        dict.set_item("id", self.id.into_inner())?;
        dict.set_item("value", self.evaluated_value())?;
        set_used_ids(&dict, self.used_decision_variable_ids())?;
        set_metadata(
            &dict,
            self.name.as_deref(),
            &self.subscripts,
            self.description.as_deref(),
            na,
        )?;
        set_parameter_columns(&dict, &self.parameters)?;
        Ok(dict.into_any())
    }
}

// ---------------------------------------------------------------------------
// ToPandasEntry implementations for sampled types (SampleSet)
// ---------------------------------------------------------------------------

// SampleSet types have dynamic per-sample columns, so they don't use ToPandasEntry.
// They use raw_entries_to_dataframe with inline dict construction + the helpers above.
