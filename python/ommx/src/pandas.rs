//! Thin wrapper around `pandas.DataFrame` for type-safe PyO3 bindings,
//! plus shared helpers for building DataFrames from domain objects.

use fnv::FnvHashMap;
use ommx::{
    ConstraintMetadata, ConstraintMetadataStore, DecisionVariableMetadata, Evaluate, IDType,
    Provenance, RemovedReason, VariableID, VariableIDSet, VariableMetadataStore,
};
use pyo3::{
    exceptions::PyValueError,
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
// IncludeFlags — gates optional column families on wide `*_df` methods
// ---------------------------------------------------------------------------

/// Which optional column families to fold into a wide `*_df` DataFrame.
///
/// `metadata` toggles the `name` / `subscripts` / `description` columns.
/// `parameters` toggles the `parameters.{key}` columns. The default
/// (`Self::default_wide()`) preserves the v2-equivalent wide shape.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct IncludeFlags {
    pub metadata: bool,
    pub parameters: bool,
}

impl IncludeFlags {
    /// Default for wide `*_df` — both metadata and parameters columns on.
    pub fn default_wide() -> Self {
        Self {
            metadata: true,
            parameters: true,
        }
    }

    /// Parse `include=[...]` arg from Python. `None` returns the wide default.
    pub fn from_optional(include: Option<Vec<String>>) -> PyResult<Self> {
        match include {
            None => Ok(Self::default_wide()),
            Some(values) => {
                let mut flags = Self::default();
                for v in &values {
                    match v.as_str() {
                        "metadata" => flags.metadata = true,
                        "parameters" => flags.parameters = true,
                        other => {
                            return Err(PyValueError::new_err(format!(
                                "unknown include flag: {other:?} (expected one of \"metadata\", \"parameters\")"
                            )));
                        }
                    }
                }
                Ok(flags)
            }
        }
    }
}

const METADATA_KEYS: &[&str] = &["name", "subscripts", "description"];

// ---------------------------------------------------------------------------
// kind= dispatch — shared by the 4 constraint sidecar accessors
// ---------------------------------------------------------------------------

/// Constraint family selector for `kind=` arguments on sidecar DataFrames.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConstraintKind {
    Regular,
    Indicator,
    OneHot,
    Sos1,
}

/// Parse the `kind=` string argument. Returns `ValueError` on unknown values.
pub fn parse_constraint_kind(kind: &str) -> PyResult<ConstraintKind> {
    match kind {
        "regular" => Ok(ConstraintKind::Regular),
        "indicator" => Ok(ConstraintKind::Indicator),
        "one_hot" => Ok(ConstraintKind::OneHot),
        "sos1" => Ok(ConstraintKind::Sos1),
        other => Err(PyValueError::new_err(format!(
            "unknown constraint kind: {other:?} (expected one of \"regular\", \"indicator\", \"one_hot\", \"sos1\")"
        ))),
    }
}

/// Index column name for the chosen constraint kind. Each kind has a
/// distinct ID space, so the qualified name keeps cross-kind joins
/// visible (`regular_constraint_id` ≠ `indicator_constraint_id` etc.).
pub fn constraint_id_col(kind: ConstraintKind) -> &'static str {
    match kind {
        ConstraintKind::Regular => "regular_constraint_id",
        ConstraintKind::Indicator => "indicator_constraint_id",
        ConstraintKind::OneHot => "one_hot_constraint_id",
        ConstraintKind::Sos1 => "sos1_constraint_id",
    }
}

// ---------------------------------------------------------------------------
// Sidecar DataFrame builders
//
// Long-format / id-indexed views over the SoA metadata stores. Each builder
// reads the store directly and produces a DataFrame with a documented column
// schema. Used by the `*_metadata_df`, `*_parameters_df`, `*_provenance_df`,
// and `*_removed_reasons_df` accessors on Instance / ParametricInstance /
// Solution / SampleSet.
// ---------------------------------------------------------------------------

/// Wide id-indexed metadata DataFrame for constraints.
///
/// One row per id from `ids`, in iteration order. Columns: `name`,
/// `subscripts`, `description`. Index column = `id_col`.
pub fn constraint_metadata_dataframe<'py, ID>(
    py: Python<'py>,
    store: &ConstraintMetadataStore<ID>,
    ids: impl Iterator<Item = ID>,
    id_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>>
where
    ID: IDType + Into<u64>,
{
    let entries: Vec<Bound<'py, PyAny>> = ids
        .map(|id| -> PyResult<_> {
            let dict = PyDict::new(py);
            dict.set_item(id_col, Into::<u64>::into(id))?;
            set_metadata(
                &dict,
                store.name(id),
                store.subscripts(id),
                store.description(id),
            )?;
            Ok(dict.into_any())
        })
        .collect::<PyResult<_>>()?;
    raw_entries_to_dataframe(py, entries, id_col)
}

/// Long-format parameters DataFrame for constraints.
///
/// One row per (id, key) pair where `store.parameters(id)` is non-empty.
/// Columns: `id_col`, `key`, `value`. Default RangeIndex (no `set_index`).
pub fn constraint_parameters_dataframe<'py, ID>(
    py: Python<'py>,
    store: &ConstraintMetadataStore<ID>,
    ids: impl Iterator<Item = ID>,
    id_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>>
where
    ID: IDType + Into<u64>,
{
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for id in ids {
        let params = store.parameters(id);
        for (key, value) in params {
            let dict = PyDict::new(py);
            dict.set_item(id_col, Into::<u64>::into(id))?;
            dict.set_item("key", key)?;
            dict.set_item("value", value)?;
            entries.push(dict.into_any());
        }
    }
    long_format_dataframe(py, entries)
}

/// Long-format provenance DataFrame for constraints.
///
/// One row per (id, step) pair where `store.provenance(id)` is non-empty.
/// Columns: `id_col`, `step` (0-based), `source_kind`
/// (`"IndicatorConstraint"` / `"OneHotConstraint"` / `"Sos1Constraint"`),
/// `source_id`. Default RangeIndex.
pub fn constraint_provenance_dataframe<'py, ID>(
    py: Python<'py>,
    store: &ConstraintMetadataStore<ID>,
    ids: impl Iterator<Item = ID>,
    id_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>>
where
    ID: IDType + Into<u64>,
{
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for id in ids {
        for (step, p) in store.provenance(id).iter().enumerate() {
            let (source_kind, source_id) = provenance_parts(p);
            let dict = PyDict::new(py);
            dict.set_item(id_col, Into::<u64>::into(id))?;
            dict.set_item("step", step as u64)?;
            dict.set_item("source_kind", source_kind)?;
            dict.set_item("source_id", source_id)?;
            entries.push(dict.into_any());
        }
    }
    long_format_dataframe(py, entries)
}

/// Long-format removed-reasons DataFrame for constraints.
///
/// One row per (id, parameter_key) pair when the removed reason has
/// parameters; ids without parameters get one row with `key`/`value` set to
/// `pandas.NA`. Columns: `id_col`, `reason`, `key`, `value`. Default
/// RangeIndex.
pub fn constraint_removed_reasons_dataframe<'py, 'a, ID>(
    py: Python<'py>,
    removed: impl Iterator<Item = (ID, &'a RemovedReason)>,
    id_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>>
where
    ID: IDType + Into<u64>,
{
    let na = get_na(py)?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for (id, reason) in removed {
        if reason.parameters.is_empty() {
            let dict = PyDict::new(py);
            dict.set_item(id_col, Into::<u64>::into(id))?;
            dict.set_item("reason", &reason.reason)?;
            dict.set_item("key", &na)?;
            dict.set_item("value", &na)?;
            entries.push(dict.into_any());
        } else {
            for (key, value) in &reason.parameters {
                let dict = PyDict::new(py);
                dict.set_item(id_col, Into::<u64>::into(id))?;
                dict.set_item("reason", &reason.reason)?;
                dict.set_item("key", key)?;
                dict.set_item("value", value)?;
                entries.push(dict.into_any());
            }
        }
    }
    long_format_dataframe(py, entries)
}

/// Wide id-indexed metadata DataFrame for decision variables.
///
/// Identical column shape to [`constraint_metadata_dataframe`], reading from
/// a [`VariableMetadataStore`] instead.
pub fn variable_metadata_dataframe<'py>(
    py: Python<'py>,
    store: &VariableMetadataStore,
    ids: impl Iterator<Item = VariableID>,
    id_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>> {
    let entries: Vec<Bound<'py, PyAny>> = ids
        .map(|id| -> PyResult<_> {
            let dict = PyDict::new(py);
            dict.set_item(id_col, Into::<u64>::into(id))?;
            set_metadata(
                &dict,
                store.name(id),
                store.subscripts(id),
                store.description(id),
            )?;
            Ok(dict.into_any())
        })
        .collect::<PyResult<_>>()?;
    raw_entries_to_dataframe(py, entries, id_col)
}

/// Long-format parameters DataFrame for decision variables.
pub fn variable_parameters_dataframe<'py>(
    py: Python<'py>,
    store: &VariableMetadataStore,
    ids: impl Iterator<Item = VariableID>,
    id_col: &str,
) -> PyResult<Bound<'py, PyDataFrame>> {
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for id in ids {
        let params = store.parameters(id);
        for (key, value) in params {
            let dict = PyDict::new(py);
            dict.set_item(id_col, Into::<u64>::into(id))?;
            dict.set_item("key", key)?;
            dict.set_item("value", value)?;
            entries.push(dict.into_any());
        }
    }
    long_format_dataframe(py, entries)
}

fn provenance_parts(p: &Provenance) -> (&'static str, u64) {
    match *p {
        Provenance::IndicatorConstraint(id) => ("IndicatorConstraint", id.into()),
        Provenance::OneHotConstraint(id) => ("OneHotConstraint", id.into()),
        Provenance::Sos1Constraint(id) => ("Sos1Constraint", id.into()),
    }
}

/// Build a long-format DataFrame from pre-built entry dicts.
///
/// No `set_index` call, so the DataFrame keeps its default RangeIndex.
/// Used by the `*_parameters_df` / `*_provenance_df` /
/// `*_removed_reasons_df` builders.
fn long_format_dataframe<'py>(
    py: Python<'py>,
    entries: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDataFrame>> {
    let pandas = py.import("pandas")?;
    let df = pandas.call_method1("DataFrame", (entries,))?;
    df.cast_into().map_err(Into::into)
}

/// Drop columns from a per-row dict according to the include flags.
///
/// `metadata` columns are dropped by name; `parameters.*` columns are
/// dropped by prefix. Missing keys are silently skipped (some impls don't
/// emit every key).
fn apply_include_filter(dict: &Bound<PyDict>, include: IncludeFlags) -> PyResult<()> {
    if !include.metadata {
        for key in METADATA_KEYS {
            if dict.contains(key)? {
                dict.del_item(key)?;
            }
        }
    }
    if !include.parameters {
        let to_drop: Vec<String> = dict
            .keys()
            .iter()
            .filter_map(|k| k.extract::<String>().ok())
            .filter(|k| k.starts_with("parameters."))
            .collect();
        for key in to_drop {
            dict.del_item(key)?;
        }
    }
    Ok(())
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

/// Pairs a borrowed item with a borrowed metadata snapshot for pandas rendering.
///
/// Used as the input to `ToPandasEntry` impls that need access to the SoA
/// metadata that no longer lives on the per-element struct.
pub struct WithMetadata<'a, T, M> {
    pub item: T,
    pub metadata: &'a M,
}

impl<'a, T, M> WithMetadata<'a, T, M> {
    pub fn new(item: T, metadata: &'a M) -> Self {
        Self { item, metadata }
    }
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
///
/// `include` selects which optional column families to keep on each row;
/// dropped columns never reach the constructed DataFrame. Pass
/// [`IncludeFlags::default_wide()`] to preserve the v2-equivalent shape.
pub fn entries_to_dataframe<'py, T: ToPandasEntry>(
    py: Python<'py>,
    items: impl Iterator<Item = T>,
    index_col: &str,
    include: IncludeFlags,
) -> PyResult<Bound<'py, PyDataFrame>> {
    let entries: Vec<Bound<'py, PyAny>> = items
        .map(|item| {
            let dict = item.to_pandas_entry(py)?;
            apply_include_filter(&dict, include)?;
            Ok(dict.into_any())
        })
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

impl<'m> ToPandasEntry for WithMetadata<'m, &ommx::DecisionVariable, DecisionVariableMetadata> {
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        let dv = self.item;
        let m = self.metadata;
        dict.set_item("id", dv.id().into_inner())?;
        set_kind(&dict, dv.kind())?;
        dict.set_item("lower", dv.bound().lower())?;
        dict.set_item("upper", dv.bound().upper())?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        match dv.substituted_value() {
            Some(v) => dict.set_item("substituted_value", v)?,
            None => dict.set_item("substituted_value", &na)?,
        }
        set_parameter_columns(&dict, &m.parameters)?;
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<'m, (ommx::ConstraintID, &ommx::Constraint), ConstraintMetadata>
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self.item;
        let m = self.metadata;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_equality(&dict, c.equality)?;
        set_function_type(&dict, &c.stage.function)?;
        set_used_ids(&dict, &c.stage.function.required_ids())?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<
        'm,
        (ommx::IndicatorConstraintID, &ommx::IndicatorConstraint),
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self.item;
        let m = self.metadata;
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
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<
        'm,
        (
            ommx::IndicatorConstraintID,
            &ommx::EvaluatedIndicatorConstraint,
        ),
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self.item;
        let m = self.metadata;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        dict.set_item("indicator_variable_id", c.indicator_variable.into_inner())?;
        set_equality(&dict, c.equality)?;
        dict.set_item("value", c.stage.evaluated_value)?;
        dict.set_item("indicator_active", c.stage.indicator_active)?;
        set_used_ids(&dict, &c.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl<'a, 'm> ToPandasEntry
    for WithMetadata<
        'm,
        WithSampleIds<
            'a,
            (
                ommx::IndicatorConstraintID,
                &'a ommx::SampledIndicatorConstraint,
            ),
        >,
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, ic) = &self.item.item;
        let m = self.metadata;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        dict.set_item("indicator_variable_id", ic.indicator_variable.into_inner())?;
        set_equality(&dict, ic.equality)?;
        set_used_ids(&dict, &ic.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        for &sample_id in self.item.sample_ids {
            let value = ic.stage.evaluated_values.get(sample_id).copied();
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

impl<'m> ToPandasEntry
    for WithMetadata<
        'm,
        (
            ommx::IndicatorConstraintID,
            &(ommx::IndicatorConstraint, ommx::RemovedReason),
        ),
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, inner) = self.item;
        let (ic, reason) = inner;
        let dict = WithMetadata::new((id, ic), self.metadata).to_pandas_entry(py)?;
        dict.set_item("removed_reason", &reason.reason)?;
        for (key, value) in &reason.parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<
        'm,
        (ommx::OneHotConstraintID, &ommx::EvaluatedOneHotConstraint),
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self.item;
        let m = self.metadata;
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        dict.set_item("feasible", c.stage.feasible)?;
        match c.stage.active_variable {
            Some(v) => dict.set_item("active_variable", v.into_inner())?,
            None => dict.set_item("active_variable", &na)?,
        }
        set_used_ids(&dict, &c.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl<'a, 'm> ToPandasEntry
    for WithMetadata<
        'm,
        WithSampleIds<'a, (ommx::OneHotConstraintID, &'a ommx::SampledOneHotConstraint)>,
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = &self.item.item;
        let m = self.metadata;
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_used_ids(&dict, &c.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        for &sample_id in self.item.sample_ids {
            let feas = c.stage.feasible.get(&sample_id).copied();
            dict.set_item(format!("feasible.{}", sample_id.into_inner()), feas)?;
            let active_col = format!("active_variable.{}", sample_id.into_inner());
            match c.stage.active_variable.get(&sample_id) {
                Some(Some(v)) => dict.set_item(active_col, v.into_inner())?,
                _ => dict.set_item(active_col, &na)?,
            }
        }
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<
        'm,
        (ommx::Sos1ConstraintID, &ommx::EvaluatedSos1Constraint),
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self.item;
        let m = self.metadata;
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        dict.set_item("feasible", c.stage.feasible)?;
        match c.stage.active_variable {
            Some(v) => dict.set_item("active_variable", v.into_inner())?,
            None => dict.set_item("active_variable", &na)?,
        }
        set_used_ids(&dict, &c.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl<'a, 'm> ToPandasEntry
    for WithMetadata<
        'm,
        WithSampleIds<'a, (ommx::Sos1ConstraintID, &'a ommx::SampledSos1Constraint)>,
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = &self.item.item;
        let m = self.metadata;
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_used_ids(&dict, &c.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        for &sample_id in self.item.sample_ids {
            let feas = c.stage.feasible.get(&sample_id).copied();
            dict.set_item(format!("feasible.{}", sample_id.into_inner()), feas)?;
            let active_col = format!("active_variable.{}", sample_id.into_inner());
            match c.stage.active_variable.get(&sample_id) {
                Some(Some(v)) => dict.set_item(active_col, v.into_inner())?,
                _ => dict.set_item(active_col, &na)?,
            }
        }
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<'m, (ommx::OneHotConstraintID, &ommx::OneHotConstraint), ConstraintMetadata>
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, one_hot) = self.item;
        let m = self.metadata;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        let vars: Vec<u64> = one_hot.variables.iter().map(|v| v.into_inner()).collect();
        dict.set_item("variables", PySet::new(py, &vars)?)?;
        dict.set_item("num_variables", vars.len())?;
        set_used_ids(&dict, &one_hot.variables)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<
        'm,
        (
            ommx::OneHotConstraintID,
            &(ommx::OneHotConstraint, ommx::RemovedReason),
        ),
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, inner) = self.item;
        let (one_hot, reason) = inner;
        let dict = WithMetadata::new((id, one_hot), self.metadata).to_pandas_entry(py)?;
        dict.set_item("removed_reason", &reason.reason)?;
        for (key, value) in &reason.parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<'m, (ommx::Sos1ConstraintID, &ommx::Sos1Constraint), ConstraintMetadata>
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, sos1) = self.item;
        let m = self.metadata;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        let vars: Vec<u64> = sos1.variables.iter().map(|v| v.into_inner()).collect();
        dict.set_item("variables", PySet::new(py, &vars)?)?;
        dict.set_item("num_variables", vars.len())?;
        set_used_ids(&dict, &sos1.variables)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<
        'm,
        (
            ommx::Sos1ConstraintID,
            &(ommx::Sos1Constraint, ommx::RemovedReason),
        ),
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, inner) = self.item;
        let (sos1, reason) = inner;
        let dict = WithMetadata::new((id, sos1), self.metadata).to_pandas_entry(py)?;
        dict.set_item("removed_reason", &reason.reason)?;
        for (key, value) in &reason.parameters {
            dict.set_item(format!("removed_reason.{key}"), value)?;
        }
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<
        'm,
        (ommx::ConstraintID, &(ommx::Constraint, ommx::RemovedReason)),
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, inner) = self.item;
        let (constraint, reason) = inner;
        let m = self.metadata;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_equality(&dict, constraint.equality)?;
        set_function_type(&dict, &constraint.stage.function)?;
        set_used_ids(&dict, &constraint.stage.function.required_ids())?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
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

impl<'m> ToPandasEntry
    for WithMetadata<'m, &ommx::EvaluatedDecisionVariable, DecisionVariableMetadata>
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dv = self.item;
        let m = self.metadata;
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", dv.id().into_inner())?;
        set_kind(&dict, *dv.kind())?;
        dict.set_item("lower", dv.bound().lower())?;
        dict.set_item("upper", dv.bound().upper())?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        // EvaluatedDecisionVariable has no substituted_value field
        dict.set_item("substituted_value", &na)?;
        dict.set_item("value", *dv.value())?;
        Ok(dict)
    }
}

impl<'m> ToPandasEntry
    for WithMetadata<'m, (ommx::ConstraintID, &ommx::EvaluatedConstraint), ConstraintMetadata>
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, c) = self.item;
        let m = self.metadata;
        let na = get_na(py)?;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_equality(&dict, c.equality)?;
        dict.set_item("value", c.stage.evaluated_value)?;
        set_used_ids(&dict, &c.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
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

impl<'a, 'm> ToPandasEntry
    for WithMetadata<
        'm,
        WithSampleIds<'a, &'a ommx::SampledDecisionVariable>,
        DecisionVariableMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dv = self.item.item;
        let m = self.metadata;
        let dict = PyDict::new(py);
        dict.set_item("id", dv.id().into_inner())?;
        set_kind(&dict, *dv.kind())?;
        dict.set_item("lower", dv.bound().lower())?;
        dict.set_item("upper", dv.bound().upper())?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        for &sample_id in self.item.sample_ids {
            let value = dv.samples().get(sample_id).copied();
            dict.set_item(sample_id.into_inner(), value)?;
        }
        Ok(dict)
    }
}

impl<'a, 'm> ToPandasEntry
    for WithMetadata<
        'm,
        WithSampleIds<'a, (ommx::ConstraintID, &'a ommx::SampledConstraint)>,
        ConstraintMetadata,
    >
{
    fn to_pandas_entry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let (id, sc) = &self.item.item;
        let m = self.metadata;
        let dict = PyDict::new(py);
        dict.set_item("id", id.into_inner())?;
        set_equality(&dict, sc.equality)?;
        set_used_ids(&dict, &sc.stage.used_decision_variable_ids)?;
        set_metadata(
            &dict,
            m.name.as_deref(),
            &m.subscripts,
            m.description.as_deref(),
        )?;
        for &sample_id in self.item.sample_ids {
            let value = sc.stage.evaluated_values.get(sample_id).copied();
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
            let value = nf.evaluated_values().get(sample_id).copied();
            dict.set_item(sample_id.into_inner(), value)?;
        }
        Ok(dict)
    }
}
