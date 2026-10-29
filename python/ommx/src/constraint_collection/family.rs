//! A family filter retains a collection, not its rows. ID normalization is
//! family-specific; row access delegates to the owning collection's lifecycle.

use super::*;
use crate::constraint_id::{
    ConstraintIDInput, IndicatorConstraintIDInput, OneHotConstraintIDInput, Sos1ConstraintIDInput,
};
use crate::constraint_kind::{constraint_kind_collection, ConstraintKind};
use pyo3::types::PyType;

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

struct FamilyKey;
impl PyStubType for FamilyKey {
    fn type_output() -> TypeInfo {
        type_variable("_K")
    }
}

// The selected family is stored on the view, so normalization must wait until
// the method has self. Reuse the same input converters as family-fixed APIs.
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
    fn normalize(self, py: Python<'_>, kind: ConstraintKind) -> PyResult<AnyConstraintID> {
        let key = self.0.bind(py);
        Ok(match kind {
            ConstraintKind::Regular => {
                ommx::ConstraintID::from(key.extract::<ConstraintIDInput>()?).into()
            }
            ConstraintKind::Indicator => {
                ommx::IndicatorConstraintID::from(key.extract::<IndicatorConstraintIDInput>()?)
                    .into()
            }
            ConstraintKind::OneHot => {
                ommx::OneHotConstraintID::from(key.extract::<OneHotConstraintIDInput>()?).into()
            }
            ConstraintKind::Sos1 => {
                ommx::Sos1ConstraintID::from(key.extract::<Sos1ConstraintIDInput>()?).into()
            }
        })
    }
}

// The parent dispatch returns a concrete constraint object. This wrapper carries
// the view's row type parameter through the generated output signature.
struct FamilyRow(Py<PyAny>);
impl PyStubType for FamilyRow {
    fn type_output() -> TypeInfo {
        type_variable("_V_co")
    }
}
impl<'py> IntoPyObject<'py> for FamilyRow {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = std::convert::Infallible;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(self.0.into_bound(py))
    }
}

enum Owner {
    Active(Py<ConstraintCollection>),
    Removed(Py<RemovedConstraintCollection>),
    Evaluated(Py<EvaluatedConstraintCollection>),
    Sampled(Py<SampledConstraintCollection>),
}

#[doc = include_str!("../docstrings/ommx/ConstraintFamilyView.md")]
#[pyclass(frozen)]
pub struct ConstraintFamilyView {
    owner: Owner,
    kind: ConstraintKind,
}
impl PyStubType for ConstraintFamilyView {
    fn type_output() -> TypeInfo {
        TypeInfo::locally_defined("ConstraintFamilyView", "ommx._ommx_rust".into())
    }
}
pyo3_stub_gen::impl_py_runtime_type!(ConstraintFamilyView);
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyClassInfo {
        pyclass_name: "ConstraintFamilyView",
        struct_id: std::any::TypeId::of::<ConstraintFamilyView>,
        getters: &[], setters: &[], module: Some("ommx._ommx_rust"),
        doc: include_str!("../docstrings/ommx/ConstraintFamilyView.md"),
        bases: &[|| <std::collections::BTreeMap<FamilyKey, FamilyRow> as PyStubType>::type_input()],
        has_eq: false, has_ord: false, has_hash: false, has_str: false, subclass: false,
    }
}

macro_rules! with_parent {
    ($this:ident, $py:ident, |$parent:ident| $body:block) => {
        match &$this.owner {
            Owner::Active(owner) => {
                let $parent = owner.borrow($py);
                $body
            }
            Owner::Removed(owner) => {
                let $parent = owner.borrow($py);
                $body
            }
            Owner::Evaluated(owner) => {
                let $parent = owner.borrow($py);
                $body
            }
            Owner::Sampled(owner) => {
                let $parent = owner.borrow($py);
                $body
            }
        }
    };
}

// Enumerate/count only the selected family's IDs, without fetching any rows.
macro_rules! with_family {
    ($this:ident, $py:ident, |$map:ident| $body:block) => {
        match &$this.owner {
            Owner::Active(owner) => {
                let parent = owner.borrow($py);
                with_model!(&parent.owner, $py, |root| {
                    constraint_kind_collection!(
                        root,
                        $this.kind,
                        [
                            constraints,
                            indicator_constraints,
                            one_hot_constraints,
                            sos1_constraints
                        ],
                        |$map| $body
                    )
                })
            }
            Owner::Removed(owner) => {
                let parent = owner.borrow($py);
                with_model!(&parent.owner, $py, |root| {
                    constraint_kind_collection!(
                        root,
                        $this.kind,
                        [
                            removed_constraints,
                            removed_indicator_constraints,
                            removed_one_hot_constraints,
                            removed_sos1_constraints
                        ],
                        |$map| $body
                    )
                })
            }
            Owner::Evaluated(owner) => {
                let parent = owner.borrow($py);
                let root = parent.owner.borrow($py);
                constraint_kind_collection!(
                    &root.inner,
                    $this.kind,
                    [
                        evaluated_constraints,
                        evaluated_indicator_constraints,
                        evaluated_one_hot_constraints,
                        evaluated_sos1_constraints
                    ],
                    |$map| $body
                )
            }
            Owner::Sampled(owner) => {
                let parent = owner.borrow($py);
                let root = parent.owner.borrow($py);
                constraint_kind_collection!(
                    &root.inner,
                    $this.kind,
                    [
                        constraints,
                        indicator_constraints,
                        one_hot_constraints,
                        sos1_constraints
                    ],
                    |$map| $body
                )
            }
        }
    };
}

#[pyclass]
struct FamilyIDIterator(std::vec::IntoIter<AnyConstraintID>);
#[pymethods]
impl FamilyIDIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn __next__(&mut self) -> Option<AnyConstraintID> {
        self.0.next()
    }
}
impl PyStubType for FamilyIDIterator {
    fn type_output() -> TypeInfo {
        view_type("Iterator", FamilyKey::type_output())
    }
}

macro_rules! family_view {
    ($name:ident, $python:literal, $type_info:expr) => {
        struct $name(Py<PyAny>);
        impl PyStubType for $name {
            fn type_output() -> TypeInfo {
                view_type($python, $type_info)
            }
        }
        impl<'py> IntoPyObject<'py> for $name {
            type Target = PyAny;
            type Output = Bound<'py, PyAny>;
            type Error = PyErr;
            fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
                py.import("collections.abc")?
                    .getattr($python)?
                    .call1((self.0,))
            }
        }
    };
}
family_view!(FamilyKeysView, "KeysView", FamilyKey::type_output());
family_view!(FamilyValuesView, "ValuesView", FamilyRow::type_output());
family_view!(FamilyItemsView, "ItemsView", {
    let mut key = FamilyKey::type_output();
    let row = FamilyRow::type_output();
    key.name = format!("{}, {}", key.name, row.name);
    key.import.extend(row.import);
    key.type_refs.extend(row.type_refs);
    key
});

impl ConstraintFamilyView {
    fn row(&self, py: Python<'_>, id: AnyConstraintID) -> PyResult<FamilyRow> {
        with_parent!(self, py, |parent| {
            Ok(FamilyRow(
                parent
                    .__getitem__(py, id)?
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
            ))
        })
    }
}
#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintFamilyView {
    /// Number of current rows in the selected family.
    fn __len__(&self, py: Python<'_>) -> usize {
        with_family!(self, py, |map| { map.len() })
    }
    /// Fetch a row using an integer or an ID of the selected family.
    fn __getitem__(&self, py: Python<'_>, key: FamilyKeyInput) -> PyResult<FamilyRow> {
        self.row(py, key.normalize(py, self.kind)?)
    }
    fn __contains__(
        &self,
        py: Python<'_>,
        #[gen_stub(override_type(type_repr = "builtins.object"))] key: FamilyKeyInput,
    ) -> PyResult<bool> {
        let id = key.normalize(py, self.kind)?;
        Ok(with_parent!(self, py, |parent| {
            parent.__contains__(py, id)
        }))
    }
    /// Snapshot this family's typed IDs now; fetch rows only when indexed.
    fn __iter__(&self, py: Python<'_>) -> FamilyIDIterator {
        let ids: Vec<_> = with_family!(self, py, |map| {
            map.keys().copied().map(AnyConstraintID::from).collect()
        });
        FamilyIDIterator(ids.into_iter())
    }
    /// Live keys view. View iterators snapshot IDs on first advance.
    fn keys(slf: Bound<'_, Self>) -> FamilyKeysView {
        FamilyKeysView(slf.into_any().unbind())
    }
    /// Live values view; rows are fetched on demand.
    fn values(slf: Bound<'_, Self>) -> FamilyValuesView {
        FamilyValuesView(slf.into_any().unbind())
    }
    /// Live items view; keys remain typed and rows are fetched on demand.
    fn items(slf: Bound<'_, Self>) -> FamilyItemsView {
        FamilyItemsView(slf.into_any().unbind())
    }
    #[gen_stub(skip)]
    #[pyo3(signature = (key, default=None))]
    fn get(
        &self,
        py: Python<'_>,
        key: FamilyKeyInput,
        default: Option<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let id = key.normalize(py, self.kind)?;
        if with_parent!(self, py, |parent| { parent.__contains__(py, id) }) {
            Ok(self.row(py, id)?.0)
        } else {
            Ok(default.unwrap_or_else(|| py.None()))
        }
    }
    fn __repr__(slf: Bound<'_, Self>) -> PyResult<String> {
        Ok(slf
            .py()
            .get_type::<pyo3::types::PyDict>()
            .call1((slf,))?
            .repr()?
            .to_str()?
            .to_owned())
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
    // A real runtime generic alias makes annotations usable without postponed evaluation.
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

pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class ConstraintFamilyView:
    @overload
    def get(self, key: pyo3_stub_gen.RustType["FamilyKeyInput"], default: None = None) -> pyo3_stub_gen.RustType["FamilyRow"] | None: ...
    @overload
    def get(self, key: pyo3_stub_gen.RustType["FamilyKeyInput"], default: pyo3_stub_gen.RustType["GetDefault"]) -> pyo3_stub_gen.RustType["FamilyRow"] | pyo3_stub_gen.RustType["GetDefault"]: ...
"# }
}

macro_rules! of_kind {
    ($collection:ident, $variant:ident) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $collection {
            /// Return a live view of one family, using the same kind strings as
            /// constraints_df: "regular", "indicator", "one_hot", or "sos1".
            /// Integer keys are promoted; IDs of another family are rejected.
            /// Rows are fetched on demand with the original collection's lifecycle.
            #[gen_stub(skip)]
            fn of_kind(slf: Bound<'_, Self>, kind: ConstraintKind) -> ConstraintFamilyView {
                ConstraintFamilyView {
                    owner: Owner::$variant(slf.unbind()),
                    kind,
                }
            }
        }
    };
}
of_kind!(ConstraintCollection, Active);
of_kind!(RemovedConstraintCollection, Removed);
of_kind!(EvaluatedConstraintCollection, Evaluated);
of_kind!(SampledConstraintCollection, Sampled);

// The parent module owns registration of its Python collection types.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ConstraintFamilyView>()?;
    m.py()
        .import("collections.abc")?
        .getattr("Mapping")?
        .call_method1("register", (m.py().get_type::<ConstraintFamilyView>(),))?;
    Ok(())
}

pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class ConstraintCollection:
    @overload
    def of_kind(self, kind: typing.Literal["regular"]) -> ConstraintFamilyView[ConstraintID, AttachedConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["indicator"]) -> ConstraintFamilyView[IndicatorConstraintID, AttachedIndicatorConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["one_hot"]) -> ConstraintFamilyView[OneHotConstraintID, AttachedOneHotConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["sos1"]) -> ConstraintFamilyView[Sos1ConstraintID, AttachedSos1Constraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
"# }
}

pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class RemovedConstraintCollection:
    @overload
    def of_kind(self, kind: typing.Literal["regular"]) -> ConstraintFamilyView[ConstraintID, RemovedConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["indicator"]) -> ConstraintFamilyView[IndicatorConstraintID, RemovedIndicatorConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["one_hot"]) -> ConstraintFamilyView[OneHotConstraintID, RemovedOneHotConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["sos1"]) -> ConstraintFamilyView[Sos1ConstraintID, RemovedSos1Constraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
"# }
}

pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class EvaluatedConstraintCollection:
    @overload
    def of_kind(self, kind: typing.Literal["regular"]) -> ConstraintFamilyView[ConstraintID, EvaluatedConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["indicator"]) -> ConstraintFamilyView[IndicatorConstraintID, EvaluatedIndicatorConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["one_hot"]) -> ConstraintFamilyView[OneHotConstraintID, EvaluatedOneHotConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["sos1"]) -> ConstraintFamilyView[Sos1ConstraintID, EvaluatedSos1Constraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
"# }
}

pyo3_stub_gen::inventory::submit! {
 pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class SampledConstraintCollection:
    @overload
    def of_kind(self, kind: typing.Literal["regular"]) -> ConstraintFamilyView[ConstraintID, SampledConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["indicator"]) -> ConstraintFamilyView[IndicatorConstraintID, SampledIndicatorConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["one_hot"]) -> ConstraintFamilyView[OneHotConstraintID, SampledOneHotConstraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
    @overload
    def of_kind(self, kind: typing.Literal["sos1"]) -> ConstraintFamilyView[Sos1ConstraintID, SampledSos1Constraint]:
        """Return a live view of one family without collecting rows. Uses the same kind strings as constraints_df. Integer keys are promoted; wrong-family IDs are rejected. The view retains this collection and its owner; row lifecycle and iteration snapshot rules match the original collection."""
        ...
"# }
}
