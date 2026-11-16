//! Read-only diagnostic snapshots. Family views share the immutable snapshot;
//! they neither retain result hosts nor repeat Rust diagnostic computation.
use super::*;
use crate::constraint_id::{
    ConstraintIDInput, IndicatorConstraintIDInput, OneHotConstraintIDInput, Sos1ConstraintIDInput,
};
use crate::constraint_kind::ConstraintFamilyInput;
use pyo3::{exceptions::PyKeyError, types::PyType};
use std::marker::PhantomData;

fn type_variable(name: &str) -> TypeInfo {
    let mut import = pyo3_stub_gen::ImportRef::Type(Default::default());
    if let pyo3_stub_gen::ImportRef::Type(reference) = &mut import {
        reference.module = "ommx._typing".into();
        reference.name = name.into();
    }
    TypeInfo {
        name: name.into(),
        source_module: None,
        import: [import].into(),
        type_refs: Default::default(),
    }
}
struct GetDefault;
struct FamilyKey;
struct FamilyValue(DiagnosticValue);
impl PyStubType for GetDefault {
    fn type_output() -> TypeInfo {
        type_variable("_T")
    }
}
impl PyStubType for FamilyKey {
    fn type_output() -> TypeInfo {
        type_variable("_K")
    }
}
impl PyStubType for FamilyValue {
    fn type_output() -> TypeInfo {
        type_variable("_D_co")
    }
}
impl<'py> IntoPyObject<'py> for FamilyValue {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        self.0.into_pyobject(py)
    }
}

struct FamilyKeyInput(Py<PyAny>);
impl<'py> FromPyObject<'_, 'py> for FamilyKeyInput {
    type Error = PyErr;
    fn extract(value: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        Ok(Self(value.to_owned().unbind()))
    }
}
impl PyStubType for FamilyKeyInput {
    fn type_input() -> TypeInfo {
        FamilyKey::type_output() | u64::type_input()
    }
    fn type_output() -> TypeInfo {
        Self::type_input()
    }
}
impl FamilyKeyInput {
    fn normalize(self, py: Python<'_>, kind: ConstraintFamilyInput) -> PyResult<AnyConstraintID> {
        let key = self.0.bind(py);
        Ok(match kind {
            ConstraintFamilyInput::Regular => {
                ommx::ConstraintID::from(key.extract::<ConstraintIDInput>()?).into()
            }
            ConstraintFamilyInput::Indicator => {
                ommx::IndicatorConstraintID::from(key.extract::<IndicatorConstraintIDInput>()?)
                    .into()
            }
            ConstraintFamilyInput::OneHot => {
                ommx::OneHotConstraintID::from(key.extract::<OneHotConstraintIDInput>()?).into()
            }
            ConstraintFamilyInput::Sos1 => {
                ommx::Sos1ConstraintID::from(key.extract::<Sos1ConstraintIDInput>()?).into()
            }
        })
    }
}
fn matches_kind(key: AnyConstraintID, kind: ConstraintFamilyInput) -> bool {
    matches!(
        (key, kind),
        (AnyConstraintID::Regular(_), ConstraintFamilyInput::Regular)
            | (
                AnyConstraintID::Indicator(_),
                ConstraintFamilyInput::Indicator
            )
            | (AnyConstraintID::OneHot(_), ConstraintFamilyInput::OneHot)
            | (AnyConstraintID::Sos1(_), ConstraintFamilyInput::Sos1)
    )
}

#[doc = include_str!("../docstrings/ommx/ViolationDiagnosticsFamilyView.md")]
#[pyclass(frozen)]
pub struct ViolationDiagnosticsFamilyView {
    entries: Arc<BTreeMap<AnyConstraintID, DiagnosticValue>>,
    kind: ConstraintFamilyInput,
}

macro_rules! mapping_stub {
    ($name:ident, $key:ty, $value:ty, $doc:expr) => {
        impl PyStubType for $name {
            fn type_output() -> TypeInfo {
                TypeInfo::locally_defined(stringify!($name), "ommx._ommx_rust".into())
            }
        }
        pyo3_stub_gen::impl_py_runtime_type!($name);
        pyo3_stub_gen::inventory::submit! {
            pyo3_stub_gen::type_info::PyClassInfo {
                pyclass_name: stringify!($name), struct_id: std::any::TypeId::of::<$name>,
                getters: &[], setters: &[], module: Some("ommx._ommx_rust"), doc: $doc,
                bases: &[|| BTreeMap::<$key, $value>::type_input()],
                has_eq: false, has_ord: false, has_hash: false, has_str: false, subclass: false,
            }
        }
    };
}
mapping_stub!(
    ViolationDiagnostics,
    AnyConstraintID,
    DiagnosticValue,
    include_str!("../docstrings/ommx/ViolationDiagnostics.md")
);
mapping_stub!(
    ViolationDiagnosticsFamilyView,
    FamilyKey,
    FamilyValue,
    include_str!("../docstrings/ommx/ViolationDiagnosticsFamilyView.md")
);

fn view_type(name: &str, mut inner: TypeInfo) -> TypeInfo {
    inner.name = format!("typing.{name}[{}]", inner.name);
    inner.import.insert("typing".into());
    inner.source_module = None;
    inner
}
struct KeysView<K>(Py<PyAny>, PhantomData<K>);
struct ValuesView<V>(Py<PyAny>, PhantomData<V>);
struct ItemsView<K, V>(Py<PyAny>, PhantomData<(K, V)>);
impl<K: PyStubType> PyStubType for KeysView<K> {
    fn type_output() -> TypeInfo {
        view_type("KeysView", K::type_output())
    }
}
impl<V: PyStubType> PyStubType for ValuesView<V> {
    fn type_output() -> TypeInfo {
        view_type("ValuesView", V::type_output())
    }
}
impl<K: PyStubType, V: PyStubType> PyStubType for ItemsView<K, V> {
    fn type_output() -> TypeInfo {
        let mut key = K::type_output();
        let value = V::type_output();
        key.name = format!("{}, {}", key.name, value.name);
        key.import.extend(value.import);
        key.type_refs.extend(value.type_refs);
        view_type("ItemsView", key)
    }
}
macro_rules! view_conversion {
    ($name:ident<$($t:ident),+>) => {
        impl<'py, $($t),+> IntoPyObject<'py> for $name<$($t),+> {
            type Target = PyAny; type Output = Bound<'py, PyAny>; type Error = PyErr;
            fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
                py.import("collections.abc")?.getattr(stringify!($name))?.call1((self.0,))
            }
        }
    };
}
view_conversion!(KeysView<K>);
view_conversion!(ValuesView<V>);
view_conversion!(ItemsView<K,V>);

#[pyclass]
struct IDIterator(std::vec::IntoIter<AnyConstraintID>);
#[pymethods]
impl IDIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn __next__(&mut self) -> Option<AnyConstraintID> {
        self.0.next()
    }
}
struct TypedIterator<K>(IDIterator, PhantomData<K>);
impl<K: PyStubType> PyStubType for TypedIterator<K> {
    fn type_output() -> TypeInfo {
        view_type("Iterator", K::type_output())
    }
}
impl<'py, K> IntoPyObject<'py> for TypedIterator<K> {
    type Target = IDIterator;
    type Output = Bound<'py, IDIterator>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        Bound::new(py, self.0)
    }
}

impl ViolationDiagnostics {
    fn normalize(&self, _: Python<'_>, key: AnyConstraintID) -> PyResult<AnyConstraintID> {
        Ok(key)
    }
    fn accepts(&self, _: AnyConstraintID) -> bool {
        true
    }
}
impl ViolationDiagnosticsFamilyView {
    fn normalize(&self, py: Python<'_>, key: FamilyKeyInput) -> PyResult<AnyConstraintID> {
        key.normalize(py, self.kind)
    }
    fn accepts(&self, key: AnyConstraintID) -> bool {
        matches_kind(key, self.kind)
    }
}
macro_rules! mapping_methods {
    ($name:ident, $input:ty, $key:ty, $value:ty, $convert:expr) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $name {
            fn __len__(&self) -> usize {
                self.entries.keys().filter(|&&id| self.accepts(id)).count()
            }
            #[gen_stub(skip)]
            fn __getitem__(&self, py: Python<'_>, key: $input) -> PyResult<$value> {
                let id = self.normalize(py, key)?;
                let value = self
                    .entries
                    .get(&id)
                    .filter(|_| self.accepts(id))
                    .ok_or_else(|| match id.into_pyobject(py) {
                        Ok(id) => PyKeyError::new_err(id.unbind()),
                        Err(e) => e,
                    })?;
                Ok(($convert)(value.clone()))
            }
            fn __contains__(
                &self,
                py: Python<'_>,
                #[gen_stub(override_type(type_repr = "builtins.object"))] key: $input,
            ) -> PyResult<bool> {
                let id = self.normalize(py, key)?;
                Ok(self.accepts(id) && self.entries.contains_key(&id))
            }
            fn __iter__(&self) -> TypedIterator<$key> {
                TypedIterator(
                    IDIterator(
                        self.entries
                            .keys()
                            .copied()
                            .filter(|&id| self.accepts(id))
                            .collect::<Vec<_>>()
                            .into_iter(),
                    ),
                    PhantomData,
                )
            }
            fn keys(slf: Bound<'_, Self>) -> KeysView<$key> {
                KeysView(slf.into_any().unbind(), PhantomData)
            }
            fn values(slf: Bound<'_, Self>) -> ValuesView<$value> {
                ValuesView(slf.into_any().unbind(), PhantomData)
            }
            fn items(slf: Bound<'_, Self>) -> ItemsView<$key, $value> {
                ItemsView(slf.into_any().unbind(), PhantomData)
            }
            #[gen_stub(skip)]
            #[pyo3(signature = (key, default=None))]
            fn get(
                &self,
                py: Python<'_>,
                key: $input,
                default: Option<Py<PyAny>>,
            ) -> PyResult<Py<PyAny>> {
                let id = self.normalize(py, key)?;
                if let Some(value) = self.entries.get(&id).filter(|_| self.accepts(id)) {
                    Ok(value.clone().into_pyobject(py)?.unbind())
                } else {
                    Ok(default.unwrap_or_else(|| py.None()))
                }
            }
            fn __eq__(slf: Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                let py = slf.py();
                if slf.is(other) {
                    return Ok(pyo3::types::PyBool::new(py, true)
                        .to_owned()
                        .into_any()
                        .unbind());
                }
                Ok(py
                    .import("collections.abc")?
                    .getattr("Mapping")?
                    .getattr("__eq__")?
                    .call1((slf, other))?
                    .unbind())
            }
        }
    };
}
mapping_methods!(
    ViolationDiagnostics,
    AnyConstraintID,
    AnyConstraintID,
    DiagnosticValue,
    std::convert::identity
);
mapping_methods!(
    ViolationDiagnosticsFamilyView,
    FamilyKeyInput,
    FamilyKey,
    FamilyValue,
    FamilyValue
);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ViolationDiagnostics {
    /// Select one family from this snapshot. Integer keys are promoted to that
    /// family's ID; typed IDs of another family are rejected.
    #[gen_stub(skip)]
    fn of_kind(&self, kind: ConstraintFamilyInput) -> ViolationDiagnosticsFamilyView {
        ViolationDiagnosticsFamilyView {
            entries: Arc::clone(&self.entries),
            kind,
        }
    }
}
#[pymethods]
impl ViolationDiagnosticsFamilyView {
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

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ViolationDiagnostics>()?;
    m.add_class::<ViolationDiagnosticsFamilyView>()?;
    let mapping = m.py().import("collections.abc")?.getattr("Mapping")?;
    mapping.call_method1("register", (m.py().get_type::<ViolationDiagnostics>(),))?;
    mapping.call_method1(
        "register",
        (m.py().get_type::<ViolationDiagnosticsFamilyView>(),),
    )?;
    Ok(())
}
pyo3_stub_gen::inventory::submit! { pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class ViolationDiagnostics:
    @overload
    def __getitem__(self, key: ConstraintID) -> RegularViolationDiagnostics: ...
    @overload
    def __getitem__(self, key: IndicatorConstraintID) -> IndicatorViolationDiagnostics: ...
    @overload
    def __getitem__(self, key: OneHotConstraintID) -> OneHotViolationDiagnostics: ...
    @overload
    def __getitem__(self, key: Sos1ConstraintID) -> Sos1ViolationDiagnostics: ...
    @overload
    def __getitem__(self, key: ConstraintIDBase) -> ViolationDiagnosticsBase: ...
    @overload
    def get(self, key: ConstraintID, default: None = None) -> RegularViolationDiagnostics | None: ...
    @overload
    def get(self, key: ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> RegularViolationDiagnostics | pyo3_stub_gen.RustType["GetDefault"]: ...
    @overload
    def get(self, key: IndicatorConstraintID, default: None = None) -> IndicatorViolationDiagnostics | None: ...
    @overload
    def get(self, key: IndicatorConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> IndicatorViolationDiagnostics | pyo3_stub_gen.RustType["GetDefault"]: ...
    @overload
    def get(self, key: OneHotConstraintID, default: None = None) -> OneHotViolationDiagnostics | None: ...
    @overload
    def get(self, key: OneHotConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> OneHotViolationDiagnostics | pyo3_stub_gen.RustType["GetDefault"]: ...
    @overload
    def get(self, key: Sos1ConstraintID, default: None = None) -> Sos1ViolationDiagnostics | None: ...
    @overload
    def get(self, key: Sos1ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> Sos1ViolationDiagnostics | pyo3_stub_gen.RustType["GetDefault"]: ...
    @overload
    def get(self, key: ConstraintIDBase, default: None = None) -> ViolationDiagnosticsBase | None: ...
    @overload
    def get(self, key: ConstraintIDBase, default: ViolationDiagnosticsBase) -> ViolationDiagnosticsBase: ...
    @overload
    def get(self, key: ConstraintIDBase, default: pyo3_stub_gen.RustType["GetDefault"]) -> ViolationDiagnosticsBase | pyo3_stub_gen.RustType["GetDefault"]: ...
    @overload
    def of_kind(self, kind: ConstraintKind[ConstraintID]) -> ViolationDiagnosticsFamilyView[ConstraintID, RegularViolationDiagnostics]: ...
    @overload
    def of_kind(self, kind: ConstraintKind[IndicatorConstraintID]) -> ViolationDiagnosticsFamilyView[IndicatorConstraintID, IndicatorViolationDiagnostics]: ...
    @overload
    def of_kind(self, kind: ConstraintKind[OneHotConstraintID]) -> ViolationDiagnosticsFamilyView[OneHotConstraintID, OneHotViolationDiagnostics]: ...
    @overload
    def of_kind(self, kind: ConstraintKind[Sos1ConstraintID]) -> ViolationDiagnosticsFamilyView[Sos1ConstraintID, Sos1ViolationDiagnostics]: ...
    @overload
    def of_kind(self, kind: ConstraintKind[_K]) -> ViolationDiagnosticsFamilyView[_K, ViolationDiagnosticsBase]: ...
"# } }
pyo3_stub_gen::inventory::submit! { pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class ViolationDiagnosticsFamilyView:
    def __getitem__(self, key: pyo3_stub_gen.RustType["FamilyKeyInput"]) -> pyo3_stub_gen.RustType["FamilyValue"]: ...
    @overload
    def get(self, key: pyo3_stub_gen.RustType["FamilyKeyInput"], default: None = None) -> pyo3_stub_gen.RustType["FamilyValue"] | None: ...
    @overload
    def get(self, key: pyo3_stub_gen.RustType["FamilyKeyInput"], default: pyo3_stub_gen.RustType["GetDefault"]) -> pyo3_stub_gen.RustType["FamilyValue"] | pyo3_stub_gen.RustType["GetDefault"]: ...
"# } }
