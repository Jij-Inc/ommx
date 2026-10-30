//! One public constraint-family selector, shared by views and transformations.
use pyo3::{exceptions::PyValueError, prelude::*, types::PyType};
use pyo3_stub_gen::{PyStubType, TypeInfo};

/// Constraint family, including regular algebraic constraints.
///
/// Known constants select precise family views. The ID type parameter records
/// that selection; generic consumers can use ConstraintIDBase. Future SDKs may
/// add families. This selector does not declare adapter or wire support.
#[pyclass(frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintKind(ommx::ConstraintKind);

fn kind_type<T: PyStubType>() -> TypeInfo {
    let mut info = TypeInfo::locally_defined("ConstraintKind", "ommx._ommx_rust".into());
    let id = T::type_output();
    info.name = format!("{}[{}]", info.name, id.name);
    info.import.extend(id.import);
    info.type_refs.extend(id.type_refs);
    info
}
impl PyStubType for ConstraintKind {
    fn type_output() -> TypeInfo {
        kind_type::<crate::constraint_base::ConstraintIDBase>()
    }
}
pyo3_stub_gen::impl_py_runtime_type!(ConstraintKind);
impl From<ommx::ConstraintKind> for ConstraintKind {
    fn from(value: ommx::ConstraintKind) -> Self {
        Self(value)
    }
}
impl From<ConstraintKind> for ommx::ConstraintKind {
    fn from(value: ConstraintKind) -> Self {
        value.0
    }
}

macro_rules! family_constant {
    ($($name:ident: $id:ty),+) => {
        #[allow(non_upper_case_globals)]
        #[pymethods]
        impl ConstraintKind {
            $(#[classattr] pub const $name: ConstraintKind = ConstraintKind(ommx::ConstraintKind::$name);)+
        }
        pyo3_stub_gen::inventory::submit! {
            pyo3_stub_gen::type_info::PyMethodsInfo {
                struct_id: std::any::TypeId::of::<ConstraintKind>,
                attrs: &[$(pyo3_stub_gen::type_info::MemberInfo {
                    name: stringify!($name), r#type: kind_type::<$id>,
                    doc: "", default: None, deprecated: None,
                }),+],
                getters: &[], setters: &[], methods: &[],
                file: file!(), line: line!(), column: column!(),
            }
        }
    }
}
family_constant!(Regular: crate::ConstraintID, Indicator: crate::IndicatorConstraintID,
                 OneHot: crate::OneHotConstraintID, Sos1: crate::Sos1ConstraintID);

pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyClassInfo {
        pyclass_name: "ConstraintKind", struct_id: std::any::TypeId::of::<ConstraintKind>,
        getters: &[], setters: &[], module: Some("ommx._ommx_rust"),
        doc: "An extensible constraint family selector. Use Regular, Indicator, OneHot or Sos1; keep a rejecting fallback for unsupported families. The type parameter records the family's ID type for precise filtered views.",
        bases: &[|| {
            let mut import = pyo3_stub_gen::ImportRef::Type(Default::default());
            if let pyo3_stub_gen::ImportRef::Type(reference) = &mut import {
                reference.module = "ommx._typing".into(); reference.name = "_C_co".into();
            }
            TypeInfo { name: "typing.Generic[_C_co]".into(), source_module: None,
                import: ["typing".into(), import].into(), type_refs: Default::default() }
        }],
        has_eq: false, has_ord: false, has_hash: false, has_str: false, subclass: false,
    }
}
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintKind {
    fn __hash__(&self) -> isize {
        self.0 as isize
    }
    fn __int__(&self) -> i32 {
        self.0 as i32
    }
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<Self>() {
            self.0 == other.0
        } else {
            other
                .extract::<i32>()
                .is_ok_and(|code| code == self.0 as i32)
        }
    }
    fn __repr__(&self) -> String {
        format!("ConstraintKind.{:?}", self.0)
    }
    fn __str__(&self) -> &'static str {
        self.0.as_str()
    }
    #[gen_stub(skip)]
    #[classmethod]
    fn __class_getitem__(cls: &Bound<'_, PyType>, args: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Ok(cls
            .py()
            .import("types")?
            .getattr("GenericAlias")?
            .call1((cls, args))?
            .unbind())
    }
}

// Sealed dispatch input: validate supported families at the Python boundary.
// New public kinds must fail here until this operation implements their semantics.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConstraintFamilyInput {
    Regular,
    Indicator,
    OneHot,
    Sos1,
}
impl ConstraintFamilyInput {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Indicator => "indicator",
            Self::OneHot => "one_hot",
            Self::Sos1 => "sos1",
        }
    }
    fn core(self) -> ommx::ConstraintKind {
        match self {
            Self::Regular => ommx::ConstraintKind::Regular,
            Self::Indicator => ommx::ConstraintKind::Indicator,
            Self::OneHot => ommx::ConstraintKind::OneHot,
            Self::Sos1 => ommx::ConstraintKind::Sos1,
        }
    }
}
impl<'py> FromPyObject<'_, 'py> for ConstraintFamilyInput {
    type Error = PyErr;
    fn extract(ob: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let kind = ob.extract::<ConstraintKind>()?;
        match kind.0 {
            ommx::ConstraintKind::Regular => Ok(Self::Regular),
            ommx::ConstraintKind::Indicator => Ok(Self::Indicator),
            ommx::ConstraintKind::OneHot => Ok(Self::OneHot),
            ommx::ConstraintKind::Sos1 => Ok(Self::Sos1),
            _ => Err(PyValueError::new_err(format!(
                "Unsupported constraint family: {kind:?}"
            ))),
        }
    }
}
impl<'py> IntoPyObject<'py> for ConstraintFamilyInput {
    type Target = ConstraintKind;
    type Output = Bound<'py, ConstraintKind>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        Bound::new(py, ConstraintKind(self.core()))
    }
}
impl PyStubType for ConstraintFamilyInput {
    fn type_output() -> TypeInfo {
        ConstraintKind::type_output()
    }
}

/// Dispatch on `ConstraintFamilyInput` and bind `coll` to the per-kind constraint
/// collection on `$container`. Shared by native family views and pandas accessors.
///
/// Each host (`Instance` / `ParametricInstance` / `Solution` / `SampleSet`)
/// passes its own accessor names because the underlying collection types
/// differ — `ConstraintCollection` for Instance/ParametricInstance,
/// `EvaluatedConstraintCollection` for Solution, `SampledConstraintCollection`
/// for SampleSet. Centralising the match shape here avoids drift when adding a
/// new constraint kind.
macro_rules! constraint_kind_collection {
    (
        $container:expr, $kind:expr,
        [$regular:ident, $indicator:ident, $one_hot:ident, $sos1:ident],
        |$coll:ident| $body:block
    ) => {
        match $kind {
            $crate::constraint_kind::ConstraintFamilyInput::Regular => {
                let $coll = $container.$regular();
                $body
            }
            $crate::constraint_kind::ConstraintFamilyInput::Indicator => {
                let $coll = $container.$indicator();
                $body
            }
            $crate::constraint_kind::ConstraintFamilyInput::OneHot => {
                let $coll = $container.$one_hot();
                $body
            }
            $crate::constraint_kind::ConstraintFamilyInput::Sos1 => {
                let $coll = $container.$sos1();
                $body
            }
        }
    };
}

pub(crate) use constraint_kind_collection;
