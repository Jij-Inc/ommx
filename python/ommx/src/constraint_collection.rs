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
enum ConstraintCollectionValue {
    Regular(crate::AttachedConstraint),
    Indicator(crate::AttachedIndicatorConstraint),
    OneHot(crate::AttachedOneHotConstraint),
    Sos1(crate::AttachedSos1Constraint),
}
impl PyStubType for ConstraintCollectionValue {
    fn type_output() -> TypeInfo {
        crate::constraint_base::ConstraintView::type_output()
    }
}

/// Live, read-only mapping of all active constraint families. Values are write-through attached handles.
///
/// Keys must be typed constraint IDs; integers raise TypeError. Missing IDs raise KeyError.
/// The view keeps its owner alive and observes subsequent owner mutations.
#[pyclass(frozen)]
pub struct ConstraintCollection {
    owner: ConstraintHost,
}
collection_stub!(ConstraintCollection, ConstraintCollectionValue, "Live, read-only mapping of all active constraint families. Values are write-through attached handles.\n\nKeys must be typed constraint IDs; integers raise TypeError. Missing IDs raise KeyError.\nThe view keeps its owner alive and observes subsequent owner mutations.");
impl ConstraintCollection {
    /// Constructed only by the owning root property; no rows are copied.
    pub(crate) fn new(owner: ConstraintHost) -> Self {
        Self { owner }
    }
    fn ids(&self, py: Python<'_>) -> Vec<AnyConstraintID> {
        with_model!(&self.owner, py, |root| {
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
        })
    }
}
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintCollection {
    fn __len__(&self, py: Python<'_>) -> usize {
        with_model!(&self.owner, py, |root| {
            root.constraints().len()
                + root.indicator_constraints().len()
                + root.one_hot_constraints().len()
                + root.sos1_constraints().len()
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
                AnyConstraintID::Regular(id) => root.constraints().contains_key(&id.into()),
                AnyConstraintID::Indicator(id) => {
                    root.indicator_constraints().contains_key(&id.into())
                }
                AnyConstraintID::OneHot(id) => root.one_hot_constraints().contains_key(&id.into()),
                AnyConstraintID::Sos1(id) => root.sos1_constraints().contains_key(&id.into()),
            }
        })
    }
    #[gen_stub(skip)]
    fn __getitem__(
        &self,
        py: Python<'_>,
        key: AnyConstraintID,
    ) -> PyResult<ConstraintCollectionValue> {
        with_model!(&self.owner, py, |root| {
            match key {
                AnyConstraintID::Regular(id) => {
                    if !root.constraints().contains_key(&id.into()) {
                        return Err(missing(key, py));
                    }
                    Ok(ConstraintCollectionValue::Regular(
                        crate::AttachedConstraint::new(self.owner.clone_ref(py), id.into()),
                    ))
                }
                AnyConstraintID::Indicator(id) => {
                    if !root.indicator_constraints().contains_key(&id.into()) {
                        return Err(missing(key, py));
                    }
                    Ok(ConstraintCollectionValue::Indicator(
                        crate::AttachedIndicatorConstraint::new(
                            self.owner.clone_ref(py),
                            id.into(),
                        ),
                    ))
                }
                AnyConstraintID::OneHot(id) => {
                    if !root.one_hot_constraints().contains_key(&id.into()) {
                        return Err(missing(key, py));
                    }
                    Ok(ConstraintCollectionValue::OneHot(
                        crate::AttachedOneHotConstraint::new(self.owner.clone_ref(py), id.into()),
                    ))
                }
                AnyConstraintID::Sos1(id) => {
                    if !root.sos1_constraints().contains_key(&id.into()) {
                        return Err(missing(key, py));
                    }
                    Ok(ConstraintCollectionValue::Sos1(
                        crate::AttachedSos1Constraint::new(self.owner.clone_ref(py), id.into()),
                    ))
                }
            }
        })
    }
}
mapping_methods!(ConstraintCollection, ConstraintCollectionValue);
pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class ConstraintCollection:
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
enum RemovedConstraintCollectionValue {
    Regular(crate::RemovedConstraint),
    Indicator(crate::RemovedIndicatorConstraint),
    OneHot(crate::RemovedOneHotConstraint),
    Sos1(crate::RemovedSos1Constraint),
}
impl PyStubType for RemovedConstraintCollectionValue {
    fn type_output() -> TypeInfo {
        crate::constraint_base::ConstraintView::type_output()
    }
}

/// Live, read-only mapping of all removed constraint families. Each access returns a snapshot.
///
/// Keys must be typed constraint IDs; integers raise TypeError. Missing IDs raise KeyError.
/// The view keeps its owner alive and observes subsequent owner mutations.
#[pyclass(frozen)]
pub struct RemovedConstraintCollection {
    owner: ConstraintHost,
}
collection_stub!(RemovedConstraintCollection, RemovedConstraintCollectionValue, "Live, read-only mapping of all removed constraint families. Each access returns a snapshot.\n\nKeys must be typed constraint IDs; integers raise TypeError. Missing IDs raise KeyError.\nThe view keeps its owner alive and observes subsequent owner mutations.");
impl RemovedConstraintCollection {
    /// Constructed only by the owning root property; no rows are copied.
    pub(crate) fn new(owner: ConstraintHost) -> Self {
        Self { owner }
    }
    fn ids(&self, py: Python<'_>) -> Vec<AnyConstraintID> {
        with_model!(&self.owner, py, |root| {
            let mut ids = Vec::new();
            ids.extend(
                root.removed_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.removed_indicator_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.removed_one_hot_constraints()
                    .keys()
                    .copied()
                    .map(AnyConstraintID::from),
            );
            ids.extend(
                root.removed_sos1_constraints()
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
impl RemovedConstraintCollection {
    fn __len__(&self, py: Python<'_>) -> usize {
        with_model!(&self.owner, py, |root| {
            root.removed_constraints().len()
                + root.removed_indicator_constraints().len()
                + root.removed_one_hot_constraints().len()
                + root.removed_sos1_constraints().len()
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
                AnyConstraintID::Regular(id) => root.removed_constraints().contains_key(&id.into()),
                AnyConstraintID::Indicator(id) => root
                    .removed_indicator_constraints()
                    .contains_key(&id.into()),
                AnyConstraintID::OneHot(id) => {
                    root.removed_one_hot_constraints().contains_key(&id.into())
                }
                AnyConstraintID::Sos1(id) => {
                    root.removed_sos1_constraints().contains_key(&id.into())
                }
            }
        })
    }
    #[gen_stub(skip)]
    fn __getitem__(
        &self,
        py: Python<'_>,
        key: AnyConstraintID,
    ) -> PyResult<RemovedConstraintCollectionValue> {
        with_model!(&self.owner, py, |root| {
            match key {
                AnyConstraintID::Regular(id) => {
                    let id = id.into();
                    let (row, reason) = root
                        .removed_constraints()
                        .get(&id)
                        .ok_or_else(|| missing(key, py))?;
                    Ok(RemovedConstraintCollectionValue::Regular(
                        crate::RemovedConstraint::from_parts(
                            row.clone(),
                            root.constraint_collection().context().collect_for(id),
                            reason.clone(),
                        ),
                    ))
                }
                AnyConstraintID::Indicator(id) => {
                    let id = id.into();
                    let (row, reason) = root
                        .removed_indicator_constraints()
                        .get(&id)
                        .ok_or_else(|| missing(key, py))?;
                    Ok(RemovedConstraintCollectionValue::Indicator(
                        crate::RemovedIndicatorConstraint::from_parts(
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
                        .removed_one_hot_constraints()
                        .get(&id)
                        .ok_or_else(|| missing(key, py))?;
                    Ok(RemovedConstraintCollectionValue::OneHot(
                        crate::RemovedOneHotConstraint::from_parts(
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
                        .removed_sos1_constraints()
                        .get(&id)
                        .ok_or_else(|| missing(key, py))?;
                    Ok(RemovedConstraintCollectionValue::Sos1(
                        crate::RemovedSos1Constraint::from_parts(
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
    RemovedConstraintCollection,
    RemovedConstraintCollectionValue
);
pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class RemovedConstraintCollection:
    @overload
    def __getitem__(self, key: ConstraintID) -> RemovedConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: IndicatorConstraintID) -> RemovedIndicatorConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: OneHotConstraintID) -> RemovedOneHotConstraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: Sos1ConstraintID) -> RemovedSos1Constraint:
        """Fetch the constraint identified by a typed ID from the current owner."""
        ...
    @overload
    def __getitem__(self, key: ConstraintIDBase) -> ConstraintView:
        ...

    @overload
    def get(self, key: ConstraintID, default: None = None) -> RemovedConstraint | None:
        ...
    @overload
    def get(self, key: ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> RemovedConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: None = None) -> RemovedIndicatorConstraint | None:
        ...
    @overload
    def get(self, key: IndicatorConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> RemovedIndicatorConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: None = None) -> RemovedOneHotConstraint | None:
        ...
    @overload
    def get(self, key: OneHotConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> RemovedOneHotConstraint | pyo3_stub_gen.RustType["GetDefault"]:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: None = None) -> RemovedSos1Constraint | None:
        ...
    @overload
    def get(self, key: Sos1ConstraintID, default: pyo3_stub_gen.RustType["GetDefault"]) -> RemovedSos1Constraint | pyo3_stub_gen.RustType["GetDefault"]:
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

/// Live, read-only mapping of all constraint result families, including removed constraints. Each access returns a snapshot.
///
/// Keys must be typed constraint IDs; integers raise TypeError. Missing IDs raise KeyError.
/// The view keeps its owner alive and observes subsequent owner mutations.
#[pyclass(frozen)]
pub struct EvaluatedConstraintCollection {
    owner: Py<crate::Solution>,
}
collection_stub!(EvaluatedConstraintCollection, EvaluatedConstraintCollectionValue, "Live, read-only mapping of all constraint result families, including removed constraints. Each access returns a snapshot.\n\nKeys must be typed constraint IDs; integers raise TypeError. Missing IDs raise KeyError.\nThe view keeps its owner alive and observes subsequent owner mutations.");
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
                    .with_removed_reason(collection.removed_reasons().get(&id).cloned()),
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
                    .with_removed_reason(collection.removed_reasons().get(&id).cloned()),
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
                    .with_removed_reason(collection.removed_reasons().get(&id).cloned()),
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
                    .with_removed_reason(collection.removed_reasons().get(&id).cloned()),
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

/// Live, read-only mapping of all constraint result families, including removed constraints. Each access returns a snapshot.
///
/// Keys must be typed constraint IDs; integers raise TypeError. Missing IDs raise KeyError.
/// The view keeps its owner alive and observes subsequent owner mutations.
#[pyclass(frozen)]
pub struct SampledConstraintCollection {
    owner: Py<crate::SampleSet>,
}
collection_stub!(SampledConstraintCollection, SampledConstraintCollectionValue, "Live, read-only mapping of all constraint result families, including removed constraints. Each access returns a snapshot.\n\nKeys must be typed constraint IDs; integers raise TypeError. Missing IDs raise KeyError.\nThe view keeps its owner alive and observes subsequent owner mutations.");
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
                    .with_removed_reason(collection.removed_reasons().get(&id).cloned()),
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
                    .with_removed_reason(collection.removed_reasons().get(&id).cloned()),
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
                    .with_removed_reason(collection.removed_reasons().get(&id).cloned()),
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
                    .with_removed_reason(collection.removed_reasons().get(&id).cloned()),
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

/// Register the read-only collection protocol without introducing Python storage.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let mapping = m.py().import("collections.abc")?.getattr("Mapping")?;
    m.add_class::<ConstraintCollection>()?;
    mapping.call_method1("register", (m.py().get_type::<ConstraintCollection>(),))?;
    m.add_class::<RemovedConstraintCollection>()?;
    mapping.call_method1(
        "register",
        (m.py().get_type::<RemovedConstraintCollection>(),),
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
