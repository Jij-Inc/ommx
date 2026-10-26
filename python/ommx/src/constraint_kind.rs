//! Shared family selection for native result queries and pandas views.

use pyo3::{exceptions::PyValueError, prelude::*};

/// Constraint family selector shared by native result queries and DataFrame
/// accessors. Selecting a family also selects its independent ID space.
///
/// The Python surface is a string literal (`"regular"`, `"indicator"`,
/// `"one_hot"`, `"sos1"`); on the Rust side we deal with this enum so
/// downstream `match` arms get exhaustiveness checking. The
/// [`FromPyObject`] / [`IntoPyObject`] / [`PyStubType`] impls bridge
/// between the two: a Python `str` is validated and converted to the
/// enum on the way in, the enum is rendered back as a Python `str` on
/// the way out, and the stub surface is `typing.Literal[...]` so
/// callers get static-typing assistance for typos at edit time.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConstraintKind {
    Regular,
    Indicator,
    OneHot,
    Sos1,
}

impl ConstraintKind {
    /// The Python string literal corresponding to this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            ConstraintKind::Regular => "regular",
            ConstraintKind::Indicator => "indicator",
            ConstraintKind::OneHot => "one_hot",
            ConstraintKind::Sos1 => "sos1",
        }
    }
}

/// Accept a Python `str` and validate it against the four known kinds.
/// Unknown values yield `ValueError` for callers that bypass the
/// `Literal[...]` stub typing (e.g. dynamically constructed strings).
impl<'py> FromPyObject<'_, 'py> for ConstraintKind {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let s: &str = ob.extract()?;
        match s {
            "regular" => Ok(ConstraintKind::Regular),
            "indicator" => Ok(ConstraintKind::Indicator),
            "one_hot" => Ok(ConstraintKind::OneHot),
            "sos1" => Ok(ConstraintKind::Sos1),
            other => Err(PyValueError::new_err(format!(
                "unknown constraint kind: {other:?} (expected one of \"regular\", \"indicator\", \"one_hot\", \"sos1\")"
            ))),
        }
    }
}

/// Render back to a Python `str` so default values like
/// `ConstraintKind::Regular` show up as `'regular'` in the generated
/// stubs (via `pyo3_stub_gen::util::fmt_py_obj`).
impl<'py> pyo3::IntoPyObject<'py> for ConstraintKind {
    type Target = pyo3::types::PyString;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(pyo3::types::PyString::new(py, self.as_str()))
    }
}

/// Surface in `__init__.pyi` as a string `Literal`. Both input and
/// output positions render identically — the wide `*_df` accessors
/// take this enum but never return it, so `type_input` is the only
/// position that matters in practice; `type_output` is provided for
/// completeness.
impl pyo3_stub_gen::PyStubType for ConstraintKind {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            name: r#"typing.Literal["regular", "indicator", "one_hot", "sos1"]"#.to_string(),
            source_module: None,
            import: ["typing".into()].into(),
            type_refs: Default::default(),
        }
    }
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        Self::type_input()
    }
}
