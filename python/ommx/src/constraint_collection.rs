//! Live Python constraint mappings. The root owns all rows and invariants;
//! a view owns only a reference to that root. Only indexing clones result rows.
//! Iteration snapshots identities, never constraints, and reads values on demand.
use crate::constraint_id::AnyConstraintID;
use crate::ConstraintHost;
use pyo3::{exceptions::PyKeyError, prelude::*};
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::marker::PhantomData;

// Imported by name so generic get signatures remain compact in generated stubs.
// This marker describes the caller's default type, not a runtime Python value.
struct GetDefault;
impl PyStubType for GetDefault {
    fn type_output() -> TypeInfo {
        // TypeRef is not re-exported by pyo3-stub-gen; infer it via ImportRef.
        let mut import = pyo3_stub_gen::ImportRef::Type(Default::default());
        if let pyo3_stub_gen::ImportRef::Type(reference) = &mut import {
            reference.module = "ommx._typing".into();
            reference.name = "_T".into();
        }
        TypeInfo {
            name: "_T".into(),
            source_module: None,
            import: [import].into(),
            type_refs: Default::default(),
        }
    }
}

fn missing(id: AnyConstraintID, py: Python<'_>) -> PyErr {
    match id.into_pyobject(py) {
        Ok(value) => PyKeyError::new_err(value.unbind()),
        Err(error) => error,
    }
}

// PyO3 classes are virtual Mapping subclasses at runtime. Record that same
// relationship in generated stubs (PyO3 cannot directly extend a Python ABC).
macro_rules! collection_stub {
    ($name:ident, $row:ident, $doc:expr) => {
        impl PyStubType for $name {
            fn type_output() -> TypeInfo {
                TypeInfo::locally_defined(stringify!($name), "ommx._ommx_rust".into())
            }
        }
        pyo3_stub_gen::impl_py_runtime_type!($name);
        pyo3_stub_gen::inventory::submit! {
            pyo3_stub_gen::type_info::PyClassInfo {
                pyclass_name: stringify!($name),
                struct_id: std::any::TypeId::of::<$name>,
                getters: &[], setters: &[], module: Some("ommx._ommx_rust"),
                doc: $doc,
                bases: &[|| <std::collections::BTreeMap<AnyConstraintID, $row> as PyStubType>::type_input()],
                has_eq: false, has_ord: false, has_hash: false, has_str: false,
                subclass: false,
            }
        }
    };
}

// Python-owned mapping views implement lazy values/items through __getitem__.
// These wrappers keep that known runtime shape visible to stub generation.
struct KeysView(Py<PyAny>);
struct ValuesView<T>(Py<PyAny>, PhantomData<T>);
struct ItemsView<T>(Py<PyAny>, PhantomData<T>);

fn view_type(name: &str, mut inner: TypeInfo) -> TypeInfo {
    inner.name = format!("typing.{name}[{}]", inner.name);
    inner.import.insert("typing".into());
    inner.source_module = None;
    inner
}
impl PyStubType for KeysView {
    fn type_output() -> TypeInfo {
        view_type("KeysView", AnyConstraintID::type_output())
    }
}
impl<T: PyStubType> PyStubType for ValuesView<T> {
    fn type_output() -> TypeInfo {
        view_type("ValuesView", T::type_output())
    }
}
impl<T: PyStubType> PyStubType for ItemsView<T> {
    fn type_output() -> TypeInfo {
        let mut inner = AnyConstraintID::type_output();
        let row = T::type_output();
        inner.name = format!("{}, {}", inner.name, row.name);
        inner.import.extend(row.import);
        inner.type_refs.extend(row.type_refs);
        view_type("ItemsView", inner)
    }
}
macro_rules! view_conversion {
    ($name:ident $(<$t:ident>)?) => {
        impl<'py $(, $t)?> IntoPyObject<'py> for $name $(<$t>)? {
            type Target = PyAny;
            type Output = Bound<'py, PyAny>;
            type Error = PyErr;
            fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
                py.import("collections.abc")?.getattr(stringify!($name))?.call1((self.0,))
            }
        }
    };
}
view_conversion!(KeysView);
view_conversion!(ValuesView<T>);
view_conversion!(ItemsView<T>);

#[pyclass]
struct ConstraintIDIterator(std::vec::IntoIter<AnyConstraintID>);
#[pymethods]
impl ConstraintIDIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn __next__(&mut self) -> Option<AnyConstraintID> {
        self.0.next()
    }
}
impl PyStubType for ConstraintIDIterator {
    fn type_output() -> TypeInfo {
        view_type("Iterator", AnyConstraintID::type_output())
    }
}

// Both model roots expose the same family accessors. Keep dispatch here so
// lookup behavior remains identical for Instance and ParametricInstance.
macro_rules! with_model {
    ($owner:expr, $py:expr, |$root:ident| $body:block) => {
        match $owner {
            ConstraintHost::Instance(owner) => {
                let borrowed = owner.borrow($py);
                let $root = &borrowed.inner;
                $body
            }
            ConstraintHost::Parametric(owner) => {
                let borrowed = owner.borrow($py);
                let $root = &borrowed.inner;
                $body
            }
        }
    };
}

macro_rules! mapping_methods {
    ($name:ident, $row:ident) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $name {
            /// Iterate a snapshot of typed IDs; constraint rows are not collected.
            fn __iter__(&self, py: Python<'_>) -> ConstraintIDIterator {
                ConstraintIDIterator(self.ids(py).into_iter())
            }
            /// Live key view. Iteration order is family order, then numeric ID.
            fn keys(slf: Bound<'_, Self>) -> KeysView {
                KeysView(slf.into_any().unbind())
            }
            /// Live value view; each row is fetched when visited.
            fn values(slf: Bound<'_, Self>) -> ValuesView<$row> {
                ValuesView(slf.into_any().unbind(), PhantomData)
            }
            /// Live item view; each row is fetched when visited.
            fn items(slf: Bound<'_, Self>) -> ItemsView<$row> {
                ItemsView(slf.into_any().unbind(), PhantomData)
            }
            // Family-specific overloads below preserve both the row type and
            // the caller's default type, with an open common-base fallback.
            #[gen_stub(skip)]
            #[pyo3(signature = (key, default=None))]
            fn get(
                &self,
                py: Python<'_>,
                key: AnyConstraintID,
                default: Option<Py<PyAny>>,
            ) -> PyResult<Py<PyAny>> {
                if self.__contains__(py, key) {
                    Ok(self
                        .__getitem__(py, key)?
                        .into_pyobject(py)?
                        .into_any()
                        .unbind())
                } else {
                    Ok(default.unwrap_or_else(|| py.None()))
                }
            }
            fn __repr__(slf: Bound<'_, Self>) -> PyResult<String> {
                let py = slf.py();
                Ok(py
                    .get_type::<pyo3::types::PyDict>()
                    .call1((slf,))?
                    .repr()?
                    .to_str()?
                    .to_owned())
            }
            fn __eq__(slf: Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                let py = slf.py();
                // Preserve reflexivity even for snapshots containing non-reflexive
                // numeric values (for example NaN). Row equality handles fresh views.
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

#[derive(IntoPyObject)]
// Root name-based extraction returns the same open row type as ID lookup.
pub(crate) enum ActiveConstraintCollectionValue {
    Regular(crate::AttachedConstraint),
    Indicator(crate::AttachedIndicatorConstraint),
    OneHot(crate::AttachedOneHotConstraint),
    Sos1(crate::AttachedSos1Constraint),
}
impl PyStubType for ActiveConstraintCollectionValue {
    fn type_output() -> TypeInfo {
        crate::constraint_base::ConstraintView::type_output()
    }
}

#[doc = include_str!("docstrings/ommx/ActiveConstraintCollection.md")]
#[pyclass(frozen)]
pub struct ActiveConstraintCollection {
    owner: ConstraintHost,
}
collection_stub!(
    ActiveConstraintCollection,
    ActiveConstraintCollectionValue,
    include_str!("docstrings/ommx/ActiveConstraintCollection.md")
);
impl ActiveConstraintCollection {
    /// Constructed only by the owning root property; no rows are copied.
    pub(crate) fn new(owner: ConstraintHost) -> Self {
        Self { owner }
    }
    /// Shared implementation of the two model roots' name-based Python mapping.
    pub(crate) fn extract_by_name(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> PyResult<crate::subscript_map::SubscriptMap<ActiveConstraintCollectionValue>> {
        let ids = with_model!(&self.owner, py, |root| {
            let mut ids = std::collections::BTreeMap::new();
            macro_rules! collect {
                ($collection:expr) => {
                    let collection = $collection;
                    let context = collection.context();
                    for id in collection.active().keys() {
                        if context.name(*id) == Some(name) {
                            let key = context.subscripts(*id).to_vec();
                            if ids
                                .insert(key.clone(), AnyConstraintID::from(*id))
                                .is_some()
                            {
                                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                                    "Duplicate subscript for constraint name {name:?}: {key:?}"
                                )));
                            }
                        }
                    }
                };
            }
            collect!(root.constraint_collection());
            collect!(root.indicator_constraint_collection());
            collect!(root.one_hot_constraint_collection());
            collect!(root.sos1_constraint_collection());
            ids
        });
        if ids.is_empty() {
            return Err(PyKeyError::new_err(format!(
                "No constraint with name '{name}' found"
            )));
        }
        let rows = ids
            .into_iter()
            .map(|(key, id)| Ok((key, self.__getitem__(py, id)?)))
            .collect::<PyResult<_>>()?;
        Ok(crate::subscript_map::SubscriptMap(rows))
    }
    fn ids(&self, py: Python<'_>) -> Vec<AnyConstraintID> {
        with_model!(&self.owner, py, |root| {
            let mut ids = Vec::new();
            ids.extend(
                root.active_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.active_indicator_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.active_one_hot_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.active_sos1_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids
        })
    }
}
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ActiveConstraintCollection {
    fn __len__(&self, py: Python<'_>) -> usize {
        with_model!(&self.owner, py, |root| {
            root.active_constraints().len()
                + root.active_indicator_constraints().len()
                + root.active_one_hot_constraints().len()
                + root.active_sos1_constraints().len()
        })
    }
    // Mapping accepts object for membership checks; extraction still rejects
    // untyped keys at runtime, just as dict can reject unhashable keys.
    fn __contains__(
        &self,
        py: Python<'_>,
        #[gen_stub(override_type(type_repr = "builtins.object"))] key: AnyConstraintID,
    ) -> bool {
        with_model!(&self.owner, py, |root| {
            match key {
                AnyConstraintID::Regular(id) => root.active_constraints().contains_key(&id.into()),
                AnyConstraintID::Indicator(id) => {
                    root.active_indicator_constraints().contains_key(&id.into())
                }
                AnyConstraintID::OneHot(id) => {
                    root.active_one_hot_constraints().contains_key(&id.into())
                }
                AnyConstraintID::Sos1(id) => {
                    root.active_sos1_constraints().contains_key(&id.into())
                }
            }
        })
    }
    #[gen_stub(skip)]
    fn __getitem__(
        &self,
        py: Python<'_>,
        key: AnyConstraintID,
    ) -> PyResult<ActiveConstraintCollectionValue> {
        with_model!(&self.owner, py, |root| {
            match key {
                AnyConstraintID::Regular(id) => {
                    if !root.active_constraints().contains_key(&id.into()) {
                        return Err(missing(key, py));
                    }
                    Ok(ActiveConstraintCollectionValue::Regular(
                        crate::AttachedConstraint::new(self.owner.clone_ref(py), id.into()),
                    ))
                }
                AnyConstraintID::Indicator(id) => {
                    if !root.active_indicator_constraints().contains_key(&id.into()) {
                        return Err(missing(key, py));
                    }
                    Ok(ActiveConstraintCollectionValue::Indicator(
                        crate::AttachedIndicatorConstraint::new(
                            self.owner.clone_ref(py),
                            id.into(),
                        ),
                    ))
                }
                AnyConstraintID::OneHot(id) => {
                    if !root.active_one_hot_constraints().contains_key(&id.into()) {
                        return Err(missing(key, py));
                    }
                    Ok(ActiveConstraintCollectionValue::OneHot(
                        crate::AttachedOneHotConstraint::new(self.owner.clone_ref(py), id.into()),
                    ))
                }
                AnyConstraintID::Sos1(id) => {
                    if !root.active_sos1_constraints().contains_key(&id.into()) {
                        return Err(missing(key, py));
                    }
                    Ok(ActiveConstraintCollectionValue::Sos1(
                        crate::AttachedSos1Constraint::new(self.owner.clone_ref(py), id.into()),
                    ))
                }
            }
        })
    }
}
mapping_methods!(ActiveConstraintCollection, ActiveConstraintCollectionValue);
pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class ActiveConstraintCollection:
    @overload
    def __getitem__(self, key: ConstraintID) -> AttachedConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: IndicatorConstraintID) -> AttachedIndicatorConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: OneHotConstraintID) -> AttachedOneHotConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: Sos1ConstraintID) -> AttachedSos1Constraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: ConstraintIDBase) -> ConstraintView:
        ...

    @overload
    def get(self, key: ConstraintID, default: None = None) -> AttachedConstraint | None:
        ...
    @overload
    def get(self, key: ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> AttachedConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: None = None) -> AttachedIndicatorConstraint | None:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> AttachedIndicatorConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: None = None) -> AttachedOneHotConstraint | None:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> AttachedOneHotConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: None = None) -> AttachedSos1Constraint | None:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> AttachedSos1Constraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: None = None) -> ConstraintView | None:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: ConstraintView) -> ConstraintView:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: pyo3_stub_gen.RustType["GetDefault"]) -> ConstraintView | pyo3_stub_gen.RustType["GetDefault"]:
        ...

"# }
}

#[derive(IntoPyObject)]
pub(crate) enum InactiveConstraintCollectionValue {
    Regular(crate::InactiveConstraint),
    Indicator(crate::InactiveIndicatorConstraint),
    OneHot(crate::InactiveOneHotConstraint),
    Sos1(crate::InactiveSos1Constraint),
}
impl PyStubType for InactiveConstraintCollectionValue {
    fn type_output() -> TypeInfo {
        crate::constraint_base::ConstraintView::type_output()
    }
}

#[doc = include_str!("docstrings/ommx/InactiveConstraintCollection.md")]
#[pyclass(frozen)]
pub struct InactiveConstraintCollection {
    owner: ConstraintHost,
}
collection_stub!(
    InactiveConstraintCollection,
    InactiveConstraintCollectionValue,
    include_str!("docstrings/ommx/InactiveConstraintCollection.md")
);
impl InactiveConstraintCollection {
    /// Constructed only by the owning root property; no rows are copied.
    pub(crate) fn new(owner: ConstraintHost) -> Self {
        Self { owner }
    }
    fn ids(&self, py: Python<'_>) -> Vec<AnyConstraintID> {
        with_model!(&self.owner, py, |root| {
            let mut ids = Vec::new();
            ids.extend(
                root.inactive_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.inactive_indicator_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.inactive_one_hot_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.inactive_sos1_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids
        })
    }
}
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl InactiveConstraintCollection {
    fn __len__(&self, py: Python<'_>) -> usize {
        with_model!(&self.owner, py, |root| {
            root.inactive_constraints().len()
                + root.inactive_indicator_constraints().len()
                + root.inactive_one_hot_constraints().len()
                + root.inactive_sos1_constraints().len()
        })
    }
    // Mapping accepts object for membership checks; extraction still rejects
    // untyped keys at runtime, just as dict can reject unhashable keys.
    fn __contains__(
        &self,
        py: Python<'_>,
        #[gen_stub(override_type(type_repr = "builtins.object"))] key: AnyConstraintID,
    ) -> bool {
        with_model!(&self.owner, py, |root| {
            match key {
                AnyConstraintID::Regular(id) => {
                    root.inactive_constraints().contains_key(&id.into())
                }
                AnyConstraintID::Indicator(id) => root
                    .inactive_indicator_constraints()
                    .contains_key(&id.into()),
                AnyConstraintID::OneHot(id) => {
                    root.inactive_one_hot_constraints().contains_key(&id.into())
                }
                AnyConstraintID::Sos1(id) => {
                    root.inactive_sos1_constraints().contains_key(&id.into())
                }
            }
        })
    }
    #[gen_stub(skip)]
    fn __getitem__(
        &self,
        py: Python<'_>,
        key: AnyConstraintID,
    ) -> PyResult<InactiveConstraintCollectionValue> {
        with_model!(&self.owner, py, |root| {
            match key {
                AnyConstraintID::Regular(id) => {
                    let id = id.into();
                    let (row, reason) = root
                        .inactive_constraints()
                        .get(&id)
                        .ok_or_else(|| missing(key, py))?;
                    Ok(InactiveConstraintCollectionValue::Regular(
                        crate::InactiveConstraint::from_parts(
                            row.clone(),
                            root.constraint_collection().context().collect_for(id),
                            reason.clone(),
                        ),
                    ))
                }
                AnyConstraintID::Indicator(id) => {
                    let id = id.into();
                    let (row, reason) = root
                        .inactive_indicator_constraints()
                        .get(&id)
                        .ok_or_else(|| missing(key, py))?;
                    Ok(InactiveConstraintCollectionValue::Indicator(
                        crate::InactiveIndicatorConstraint::from_parts(
                            row.clone(),
                            root.indicator_constraint_collection()
                                .context()
                                .collect_for(id),
                            reason.clone(),
                        ),
                    ))
                }
                AnyConstraintID::OneHot(id) => {
                    let id = id.into();
                    let (row, reason) = root
                        .inactive_one_hot_constraints()
                        .get(&id)
                        .ok_or_else(|| missing(key, py))?;
                    Ok(InactiveConstraintCollectionValue::OneHot(
                        crate::InactiveOneHotConstraint::from_parts(
                            row.clone(),
                            root.one_hot_constraint_collection()
                                .context()
                                .collect_for(id),
                            reason.clone(),
                        ),
                    ))
                }
                AnyConstraintID::Sos1(id) => {
                    let id = id.into();
                    let (row, reason) = root
                        .inactive_sos1_constraints()
                        .get(&id)
                        .ok_or_else(|| missing(key, py))?;
                    Ok(InactiveConstraintCollectionValue::Sos1(
                        crate::InactiveSos1Constraint::from_parts(
                            row.clone(),
                            root.sos1_constraint_collection().context().collect_for(id),
                            reason.clone(),
                        ),
                    ))
                }
            }
        })
    }
}
mapping_methods!(
    InactiveConstraintCollection,
    InactiveConstraintCollectionValue
);
pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class InactiveConstraintCollection:
    @overload
    def __getitem__(self, key: ConstraintID) -> InactiveConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: IndicatorConstraintID) -> InactiveIndicatorConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: OneHotConstraintID) -> InactiveOneHotConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: Sos1ConstraintID) -> InactiveSos1Constraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: ConstraintIDBase) -> ConstraintView:
        ...

    @overload
    def get(self, key: ConstraintID, default: None = None) -> InactiveConstraint | None:
        ...
    @overload
    def get(self, key: ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> InactiveConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: None = None) -> InactiveIndicatorConstraint | None:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> InactiveIndicatorConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: None = None) -> InactiveOneHotConstraint | None:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> InactiveOneHotConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: None = None) -> InactiveSos1Constraint | None:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> InactiveSos1Constraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: None = None) -> ConstraintView | None:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: ConstraintView) -> ConstraintView:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: pyo3_stub_gen.RustType["GetDefault"]) -> ConstraintView | pyo3_stub_gen.RustType["GetDefault"]:
        ...

"# }
}

pub(crate) enum ConstraintCollectionValue {
    Active(ActiveConstraintCollectionValue),
    Inactive(Box<InactiveConstraintCollectionValue>),
}
impl<'py> IntoPyObject<'py> for ConstraintCollectionValue {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        match self {
            Self::Active(row) => row.into_pyobject(py),
            Self::Inactive(row) => (*row).into_pyobject(py),
        }
    }
}
impl PyStubType for ConstraintCollectionValue {
    fn type_output() -> TypeInfo {
        crate::constraint_base::ConstraintView::type_output()
    }
}

/// Live mapping of every retained constraint, independent of activeness.
///
/// IDs identify the family, not the state. Lookups return an attached active
/// row or an independent inactive snapshot; inspect activeness at runtime.
#[pyclass(frozen)]
pub struct ConstraintCollection {
    owner: ConstraintHost,
}
collection_stub!(
    ConstraintCollection,
    ConstraintCollectionValue,
    "Live mapping of all retained constraints. IDs do not determine activeness."
);
impl ConstraintCollection {
    pub(crate) fn new(owner: ConstraintHost) -> Self {
        Self { owner }
    }
    fn ids(&self, py: Python<'_>) -> Vec<AnyConstraintID> {
        with_model!(&self.owner, py, |_root| {
            let mut ids = ActiveConstraintCollection::new(self.owner.clone_ref(py)).ids(py);
            ids.extend(InactiveConstraintCollection::new(self.owner.clone_ref(py)).ids(py));
            ids.sort_by_key(|id| match id {
                AnyConstraintID::Regular(id) => (0, ommx::ConstraintID::from(*id).into_inner()),
                AnyConstraintID::Indicator(id) => {
                    (1, ommx::IndicatorConstraintID::from(*id).into_inner())
                }
                AnyConstraintID::OneHot(id) => {
                    (2, ommx::OneHotConstraintID::from(*id).into_inner())
                }
                AnyConstraintID::Sos1(id) => (3, ommx::Sos1ConstraintID::from(*id).into_inner()),
            });
            ids
        })
    }
}
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintCollection {
    fn __len__(&self, py: Python<'_>) -> usize {
        with_model!(&self.owner, py, |_root| {
            ActiveConstraintCollection::new(self.owner.clone_ref(py)).__len__(py)
                + InactiveConstraintCollection::new(self.owner.clone_ref(py)).__len__(py)
        })
    }
    fn __contains__(
        &self,
        py: Python<'_>,
        #[gen_stub(override_type(type_repr = "builtins.object"))] key: AnyConstraintID,
    ) -> bool {
        with_model!(&self.owner, py, |_root| {
            ActiveConstraintCollection::new(self.owner.clone_ref(py)).__contains__(py, key)
                || InactiveConstraintCollection::new(self.owner.clone_ref(py)).__contains__(py, key)
        })
    }
    fn __getitem__(
        &self,
        py: Python<'_>,
        key: AnyConstraintID,
    ) -> PyResult<ConstraintCollectionValue> {
        with_model!(&self.owner, py, |_root| {
            let active = ActiveConstraintCollection::new(self.owner.clone_ref(py));
            if active.__contains__(py, key) {
                active
                    .__getitem__(py, key)
                    .map(ConstraintCollectionValue::Active)
            } else {
                InactiveConstraintCollection::new(self.owner.clone_ref(py))
                    .__getitem__(py, key)
                    .map(|row| ConstraintCollectionValue::Inactive(Box::new(row)))
            }
        })
    }
}
mapping_methods!(ConstraintCollection, ConstraintCollectionValue);
pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class ConstraintCollection:
    @overload
    def get(self, key: ConstraintIDBase, default: None = None) -> ConstraintView | None: ...
    @overload
    def get(self, key: ConstraintIDBase, default: ConstraintView) -> ConstraintView: ...
    @overload
    def get(self, key: ConstraintIDBase, default: pyo3_stub_gen.RustType["GetDefault"]) -> ConstraintView | pyo3_stub_gen.RustType["GetDefault"]: ...
"# }
}

#[derive(IntoPyObject)]
enum EvaluatedConstraintCollectionValue {
    Regular(crate::EvaluatedConstraint),
    Indicator(crate::EvaluatedIndicatorConstraint),
    OneHot(crate::EvaluatedOneHotConstraint),
    Sos1(crate::EvaluatedSos1Constraint),
}
impl PyStubType for EvaluatedConstraintCollectionValue {
    fn type_output() -> TypeInfo {
        crate::constraint_base::ConstraintView::type_output()
    }
}

#[doc = include_str!("docstrings/ommx/EvaluatedConstraintCollection.md")]
#[pyclass(frozen)]
pub struct EvaluatedConstraintCollection {
    owner: Py<crate::Solution>,
}
collection_stub!(
    EvaluatedConstraintCollection,
    EvaluatedConstraintCollectionValue,
    include_str!("docstrings/ommx/EvaluatedConstraintCollection.md")
);
impl EvaluatedConstraintCollection {
    /// Constructed only by the owning root property; no rows are copied.
    pub(crate) fn new(owner: Py<crate::Solution>) -> Self {
        Self { owner }
    }
    fn ids(&self, py: Python<'_>) -> Vec<AnyConstraintID> {
        let root = &self.owner.borrow(py).inner;
        let mut ids = Vec::new();
        ids.extend(
            root.evaluated_constraints()
                .keys()
                .copied()
                .map(AnyConstraintID::from),
        );
        ids.extend(
            root.evaluated_indicator_constraints()
                .keys()
                .copied()
                .map(AnyConstraintID::from),
        );
        ids.extend(
            root.evaluated_one_hot_constraints()
                .keys()
                .copied()
                .map(AnyConstraintID::from),
        );
        ids.extend(
            root.evaluated_sos1_constraints()
                .keys()
                .copied()
                .map(AnyConstraintID::from),
        );
        ids
    }
}
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedConstraintCollection {
    fn __len__(&self, py: Python<'_>) -> usize {
        let root = &self.owner.borrow(py).inner;
        root.evaluated_constraints().len()
            + root.evaluated_indicator_constraints().len()
            + root.evaluated_one_hot_constraints().len()
            + root.evaluated_sos1_constraints().len()
    }
    // Mapping accepts object for membership checks; extraction still rejects
    // untyped keys at runtime, just as dict can reject unhashable keys.
    fn __contains__(
        &self,
        py: Python<'_>,
        #[gen_stub(override_type(type_repr = "builtins.object"))] key: AnyConstraintID,
    ) -> bool {
        let root = &self.owner.borrow(py).inner;
        match key {
            AnyConstraintID::Regular(id) => root.evaluated_constraints().contains_key(&id.into()),
            AnyConstraintID::Indicator(id) => root
                .evaluated_indicator_constraints()
                .contains_key(&id.into()),
            AnyConstraintID::OneHot(id) => root
                .evaluated_one_hot_constraints()
                .contains_key(&id.into()),
            AnyConstraintID::Sos1(id) => root.evaluated_sos1_constraints().contains_key(&id.into()),
        }
    }
    #[gen_stub(skip)]
    fn __getitem__(
        &self,
        py: Python<'_>,
        key: AnyConstraintID,
    ) -> PyResult<EvaluatedConstraintCollectionValue> {
        let root = &self.owner.borrow(py).inner;
        match key {
            AnyConstraintID::Regular(id) => {
                let id = id.into();
                let collection = root.evaluated_constraints();
                let row = collection.get(&id).ok_or_else(|| missing(key, py))?;
                Ok(EvaluatedConstraintCollectionValue::Regular(
                    crate::EvaluatedConstraint::from_parts(
                        row.clone(),
                        collection.context().collect_for(id),
                    )
                    .with_inactive_reason(collection.inactive_metadata().get(&id).cloned()),
                ))
            }
            AnyConstraintID::Indicator(id) => {
                let id = id.into();
                let collection = root.evaluated_indicator_constraints();
                let row = collection.get(&id).ok_or_else(|| missing(key, py))?;
                Ok(EvaluatedConstraintCollectionValue::Indicator(
                    crate::EvaluatedIndicatorConstraint::from_parts(
                        row.clone(),
                        collection.context().collect_for(id),
                    )
                    .with_inactive_reason(collection.inactive_metadata().get(&id).cloned()),
                ))
            }
            AnyConstraintID::OneHot(id) => {
                let id = id.into();
                let collection = root.evaluated_one_hot_constraints();
                let row = collection.get(&id).ok_or_else(|| missing(key, py))?;
                Ok(EvaluatedConstraintCollectionValue::OneHot(
                    crate::EvaluatedOneHotConstraint::from_parts(
                        row.clone(),
                        collection.context().collect_for(id),
                    )
                    .with_inactive_reason(collection.inactive_metadata().get(&id).cloned()),
                ))
            }
            AnyConstraintID::Sos1(id) => {
                let id = id.into();
                let collection = root.evaluated_sos1_constraints();
                let row = collection.get(&id).ok_or_else(|| missing(key, py))?;
                Ok(EvaluatedConstraintCollectionValue::Sos1(
                    crate::EvaluatedSos1Constraint::from_parts(
                        row.clone(),
                        collection.context().collect_for(id),
                    )
                    .with_inactive_reason(collection.inactive_metadata().get(&id).cloned()),
                ))
            }
        }
    }
}
mapping_methods!(
    EvaluatedConstraintCollection,
    EvaluatedConstraintCollectionValue
);
pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class EvaluatedConstraintCollection:
    @overload
    def __getitem__(self, key: ConstraintID) -> EvaluatedConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: IndicatorConstraintID) -> EvaluatedIndicatorConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: OneHotConstraintID) -> EvaluatedOneHotConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: Sos1ConstraintID) -> EvaluatedSos1Constraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: ConstraintIDBase) -> ConstraintView:
        ...

    @overload
    def get(self, key: ConstraintID, default: None = None) -> EvaluatedConstraint | None:
        ...
    @overload
    def get(self, key: ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> EvaluatedConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: None = None) -> EvaluatedIndicatorConstraint | None:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> EvaluatedIndicatorConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: None = None) -> EvaluatedOneHotConstraint | None:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> EvaluatedOneHotConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: None = None) -> EvaluatedSos1Constraint | None:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> EvaluatedSos1Constraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: None = None) -> ConstraintView | None:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: ConstraintView) -> ConstraintView:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: pyo3_stub_gen.RustType["GetDefault"]) -> ConstraintView | pyo3_stub_gen.RustType["GetDefault"]:
        ...

"# }
}

#[derive(IntoPyObject)]
enum SampledConstraintCollectionValue {
    Regular(crate::SampledConstraint),
    Indicator(crate::SampledIndicatorConstraint),
    OneHot(crate::SampledOneHotConstraint),
    Sos1(crate::SampledSos1Constraint),
}
impl PyStubType for SampledConstraintCollectionValue {
    fn type_output() -> TypeInfo {
        crate::constraint_base::ConstraintView::type_output()
    }
}

#[doc = include_str!("docstrings/ommx/SampledConstraintCollection.md")]
#[pyclass(frozen)]
pub struct SampledConstraintCollection {
    owner: Py<crate::SampleSet>,
}
collection_stub!(
    SampledConstraintCollection,
    SampledConstraintCollectionValue,
    include_str!("docstrings/ommx/SampledConstraintCollection.md")
);
impl SampledConstraintCollection {
    /// Constructed only by the owning root property; no rows are copied.
    pub(crate) fn new(owner: Py<crate::SampleSet>) -> Self {
        Self { owner }
    }
    fn ids(&self, py: Python<'_>) -> Vec<AnyConstraintID> {
        let root = &self.owner.borrow(py).inner;
        let mut ids = Vec::new();
        ids.extend(
            root.constraints()
                .keys()
                .copied()
                .map(AnyConstraintID::from),
        );
        ids.extend(
            root.indicator_constraints()
                .keys()
                .copied()
                .map(AnyConstraintID::from),
        );
        ids.extend(
            root.one_hot_constraints()
                .keys()
                .copied()
                .map(AnyConstraintID::from),
        );
        ids.extend(
            root.sos1_constraints()
                .keys()
                .copied()
                .map(AnyConstraintID::from),
        );
        ids
    }
}
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledConstraintCollection {
    fn __len__(&self, py: Python<'_>) -> usize {
        let root = &self.owner.borrow(py).inner;
        root.constraints().len()
            + root.indicator_constraints().len()
            + root.one_hot_constraints().len()
            + root.sos1_constraints().len()
    }
    // Mapping accepts object for membership checks; extraction still rejects
    // untyped keys at runtime, just as dict can reject unhashable keys.
    fn __contains__(
        &self,
        py: Python<'_>,
        #[gen_stub(override_type(type_repr = "builtins.object"))] key: AnyConstraintID,
    ) -> bool {
        let root = &self.owner.borrow(py).inner;
        match key {
            AnyConstraintID::Regular(id) => root.constraints().contains_key(&id.into()),
            AnyConstraintID::Indicator(id) => root.indicator_constraints().contains_key(&id.into()),
            AnyConstraintID::OneHot(id) => root.one_hot_constraints().contains_key(&id.into()),
            AnyConstraintID::Sos1(id) => root.sos1_constraints().contains_key(&id.into()),
        }
    }
    #[gen_stub(skip)]
    fn __getitem__(
        &self,
        py: Python<'_>,
        key: AnyConstraintID,
    ) -> PyResult<SampledConstraintCollectionValue> {
        let root = &self.owner.borrow(py).inner;
        match key {
            AnyConstraintID::Regular(id) => {
                let id = id.into();
                let collection = root.constraints();
                let row = collection.get(&id).ok_or_else(|| missing(key, py))?;
                Ok(SampledConstraintCollectionValue::Regular(
                    crate::SampledConstraint::from_parts(
                        row.clone(),
                        collection.context().collect_for(id),
                    )
                    .with_inactive_reason(collection.inactive_metadata().get(&id).cloned()),
                ))
            }
            AnyConstraintID::Indicator(id) => {
                let id = id.into();
                let collection = root.indicator_constraints();
                let row = collection.get(&id).ok_or_else(|| missing(key, py))?;
                Ok(SampledConstraintCollectionValue::Indicator(
                    crate::SampledIndicatorConstraint::from_parts(
                        row.clone(),
                        collection.context().collect_for(id),
                    )
                    .with_inactive_reason(collection.inactive_metadata().get(&id).cloned()),
                ))
            }
            AnyConstraintID::OneHot(id) => {
                let id = id.into();
                let collection = root.one_hot_constraints();
                let row = collection.get(&id).ok_or_else(|| missing(key, py))?;
                Ok(SampledConstraintCollectionValue::OneHot(
                    crate::SampledOneHotConstraint::from_parts(
                        row.clone(),
                        collection.context().collect_for(id),
                    )
                    .with_inactive_reason(collection.inactive_metadata().get(&id).cloned()),
                ))
            }
            AnyConstraintID::Sos1(id) => {
                let id = id.into();
                let collection = root.sos1_constraints();
                let row = collection.get(&id).ok_or_else(|| missing(key, py))?;
                Ok(SampledConstraintCollectionValue::Sos1(
                    crate::SampledSos1Constraint::from_parts(
                        row.clone(),
                        collection.context().collect_for(id),
                    )
                    .with_inactive_reason(collection.inactive_metadata().get(&id).cloned()),
                ))
            }
        }
    }
}
mapping_methods!(
    SampledConstraintCollection,
    SampledConstraintCollectionValue
);
pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class SampledConstraintCollection:
    @overload
    def __getitem__(self, key: ConstraintID) -> SampledConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: IndicatorConstraintID) -> SampledIndicatorConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: OneHotConstraintID) -> SampledOneHotConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: Sos1ConstraintID) -> SampledSos1Constraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: ConstraintIDBase) -> ConstraintView:
        ...

    @overload
    def get(self, key: ConstraintID, default: None = None) -> SampledConstraint | None:
        ...
    @overload
    def get(self, key: ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> SampledConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: None = None) -> SampledIndicatorConstraint | None:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> SampledIndicatorConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: None = None) -> SampledOneHotConstraint | None:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> SampledOneHotConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: None = None) -> SampledSos1Constraint | None:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> SampledSos1Constraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: None = None) -> ConstraintView | None:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: ConstraintView) -> ConstraintView:
        ...
    @overload
    def get(self, key: ConstraintIDBase, default: pyo3_stub_gen.RustType["GetDefault"]) -> ConstraintView | pyo3_stub_gen.RustType["GetDefault"]:
        ...

"# }
}

mod family;

/// Register the read-only collection protocol without introducing Python storage.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    family::register(m)?;
    let mapping = m.py().import("collections.abc")?.getattr("Mapping")?;
    m.add_class::<ConstraintCollection>()?;
    mapping.call_method1("register", (m.py().get_type::<ConstraintCollection>(),))?;
    m.add_class::<ActiveConstraintCollection>()?;
    mapping.call_method1(
        "register",
        (m.py().get_type::<ActiveConstraintCollection>(),),
    )?;
    m.add_class::<InactiveConstraintCollection>()?;
    mapping.call_method1(
        "register",
        (m.py().get_type::<InactiveConstraintCollection>(),),
    )?;
    m.add_class::<EvaluatedConstraintCollection>()?;
    mapping.call_method1(
        "register",
        (m.py().get_type::<EvaluatedConstraintCollection>(),),
    )?;
    m.add_class::<SampledConstraintCollection>()?;
    mapping.call_method1(
        "register",
        (m.py().get_type::<SampledConstraintCollection>(),),
    )?;
    Ok(())
}
