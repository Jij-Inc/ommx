//! Explicit pandas schemas, owned by the Python presentation layer.
//!
//! Declare the complete ordered schema before reading rows. Dynamic metadata
//! keys and sample IDs must be resolved by the caller, not discovered here.
//! Construct columns directly at their target dtype: inference followed by
//! `astype` can already have rounded a large integer in a column with missing
//! values. This module neither derives domain facts nor mutates their owners.

use super::{get_na, PyDataFrame};
use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyDict, PyList},
};

/// A column label. Integer labels support SampleSet's per-sample columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnName<'a> {
    Text(&'a str),
    Integer(u64),
}

impl<'a> From<&'a str> for ColumnName<'a> {
    fn from(value: &'a str) -> Self {
        Self::Text(value)
    }
}

impl From<u64> for ColumnName<'_> {
    fn from(value: u64) -> Self {
        Self::Integer(value)
    }
}

impl ColumnName<'_> {
    fn to_python(self, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        match self {
            Self::Text(value) => Ok(value.into_pyobject(py)?.into_any()),
            Self::Integer(value) => Ok(value.into_pyobject(py)?.into_any()),
        }
    }
}

/// One column's label and pandas dtype spelling.
///
/// Use `string` for nullable strings backed by Python storage, `object` for
/// domain objects/collections, NumPy dtype names for required scalar values,
/// and `Int64`/`UInt64`/`Float64`/`boolean` for nullable scalars. Missing keys
/// become `pd.NA`; object columns otherwise retain their values unchanged.
/// For `Float64`, only `None`/`pd.NA` are absent; a present NaN or infinity
/// remains a value. Column labels use an object Index on pandas 2 and 3.
#[derive(Clone, Copy, Debug)]
pub struct Column<'a> {
    name: ColumnName<'a>,
    dtype: &'a str,
}

impl<'a> Column<'a> {
    pub fn new(name: impl Into<ColumnName<'a>>, dtype: &'a str) -> Self {
        Self {
            name: name.into(),
            dtype,
        }
    }
}

/// Ordered columns and an optional column to use as the named index.
///
/// `build` rejects duplicate declarations, undeclared row keys, and an index
/// absent from the declarations. Without an index column, it returns an unnamed
/// RangeIndex. Empty inputs follow exactly the same construction path.
pub struct DataFrameSchema<'a> {
    columns: &'a [Column<'a>],
    index: Option<ColumnName<'a>>,
}

impl<'a> DataFrameSchema<'a> {
    pub fn new(columns: &'a [Column<'a>], index: Option<ColumnName<'a>>) -> Self {
        Self { columns, index }
    }

    pub fn build<'py>(
        &self,
        py: Python<'py>,
        rows: &[Bound<'py, PyDict>],
    ) -> PyResult<Bound<'py, PyDataFrame>> {
        let pandas = py.import("pandas")?;
        let names = PyDict::new(py);
        for column in self.columns {
            let name = column.name.to_python(py)?;
            if names.contains(&name)? {
                return Err(PyValueError::new_err(format!(
                    "duplicate DataFrame column: {:?}",
                    column.name
                )));
            }
            names.set_item(name, py.None())?;
        }
        if let Some(index) = self.index {
            if !names.contains(index.to_python(py)?)? {
                return Err(PyValueError::new_err(
                    "DataFrame index must be a declared column",
                ));
            }
        }
        for row in rows {
            for key in row.keys() {
                if !names.contains(&key)? {
                    return Err(PyValueError::new_err(format!(
                        "undeclared DataFrame column: {}",
                        key.repr()?
                    )));
                }
            }
        }
        let data = PyDict::new(py);
        let na = get_na(py)?;
        for column in self.columns {
            let name = column.name.to_python(py)?;
            let values = PyList::empty(py);
            for row in rows {
                values.append(row.get_item(&name)?.unwrap_or_else(|| na.clone()))?;
            }
            let kwargs = PyDict::new(py);
            let values = match column.dtype {
                "string" => {
                    let dtype_args = PyDict::new(py);
                    dtype_args.set_item("storage", "python")?;
                    kwargs.set_item(
                        "dtype",
                        pandas.call_method("StringDtype", (), Some(&dtype_args))?,
                    )?;
                    values.into_any()
                }
                "Float64" => {
                    // FloatingArray's explicit mask preserves a present NaN.
                    // pd.array/Series(..., dtype="Float64") masks NaN as missing.
                    let numpy = py.import("numpy")?;
                    let numbers = PyList::empty(py);
                    let mask = PyList::empty(py);
                    for value in values {
                        let missing = value.is_none() || value.is(&na);
                        mask.append(missing)?;
                        if missing {
                            numbers.append(0.0)?;
                        } else {
                            numbers.append(value)?;
                        }
                    }
                    let numbers = numpy.call_method1("array", (numbers, "float64"))?;
                    let mask = numpy.call_method1("array", (mask, "bool"))?;
                    pandas
                        .getattr("arrays")?
                        .call_method1("FloatingArray", (numbers, mask))?
                }
                dtype => {
                    kwargs.set_item("dtype", dtype)?;
                    values.into_any()
                }
            };
            let series = pandas.call_method("Series", (values,), Some(&kwargs))?;
            data.set_item(name, series)?;
        }
        let kwargs = PyDict::new(py);
        // Preserve row count even for an explicitly zero-column schema.
        kwargs.set_item("index", pandas.call_method1("RangeIndex", (rows.len(),))?)?;
        // Column labels themselves must not inherit pandas 3 string inference.
        let column_args = PyDict::new(py);
        column_args.set_item("dtype", "object")?;
        kwargs.set_item(
            "columns",
            pandas.call_method("Index", (names.keys(),), Some(&column_args))?,
        )?;
        let mut frame = pandas.call_method("DataFrame", (data,), Some(&kwargs))?;
        if let Some(index) = self.index {
            frame = frame.call_method1("set_index", (index.to_python(py)?,))?;
        }
        frame.cast_into().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
