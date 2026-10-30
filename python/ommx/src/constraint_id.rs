//! Family-specific identities for native constraint-result dictionaries.
//!
//! These are not Python int subclasses: IDs from different constraint families
//! must remain unequal even when their numerical values coincide. Conversion to
//! raw integers is confined to internal serialization boundaries.

use crate::constraint_base::ConstraintIDBase;
use pyo3::{
    conversion::FromPyObjectOwned,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyBool, PyDict, PyInt, PyMapping, PySet, PyType},
};
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::collections::{btree_map, btree_set, BTreeMap, BTreeSet};

pub(crate) trait ConstraintIDInputType: PyStubType {
    type ID: PyStubType;
}

/// Set input with covariant element typing and typed-ID output. Unlike map
/// entries, repeated identities in a set can safely collapse during promotion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConstraintSetInput<K>(BTreeSet<K>);

impl<'py, K: FromPyObjectOwned<'py> + Ord> FromPyObject<'_, 'py> for ConstraintSetInput<K> {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let set_type = value.py().import("collections.abc")?.getattr("Set")?;
        if !value.is_instance(&set_type)? {
            return Err(PyTypeError::new_err("Expected a set of constraint IDs"));
        }
        let mut result = BTreeSet::new();
        for item in value.try_iter()? {
            result.insert(item?.extract().map_err(Into::into)?);
        }
        Ok(Self(result))
    }
}

impl<K> IntoIterator for ConstraintSetInput<K> {
    type Item = K;
    type IntoIter = btree_set::IntoIter<K>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Ord> FromIterator<K> for ConstraintSetInput<K> {
    fn from_iter<T: IntoIterator<Item = K>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<'py, K: IntoPyObject<'py> + Ord> IntoPyObject<'py> for ConstraintSetInput<K> {
    type Target = PySet;
    type Output = Bound<'py, PySet>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        self.0.into_pyobject(py)
    }
}

impl<K: ConstraintIDInputType> PyStubType for ConstraintSetInput<K> {
    fn type_input() -> TypeInfo {
        let mut info = K::type_input();
        info.name = format!("typing.AbstractSet[{}]", info.name);
        info.import.insert("typing".into());
        info.source_module = None;
        info
    }

    fn type_output() -> TypeInfo {
        BTreeSet::<K::ID>::type_output()
    }
}

/// Normalize mapping keys without silently overwriting two entries that become
/// the same ID. The stub accepts homogeneous and mixed mappings because Python
/// Mapping keys are invariant in their type parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConstraintMapInput<K, V>(BTreeMap<K, V>);

impl<K, V> Default for ConstraintMapInput<K, V> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<'py, K, V> FromPyObject<'_, 'py> for ConstraintMapInput<K, V>
where
    K: FromPyObjectOwned<'py> + Ord + std::fmt::Display,
    V: FromPyObjectOwned<'py>,
{
    type Error = PyErr;

    fn extract(value: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let mapping = value.cast::<PyMapping>()?;
        let mut result = BTreeMap::new();
        for item in mapping.items()?.iter() {
            let (id, row): (K, V) = item.extract()?;
            match result.entry(id) {
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(row);
                }
                btree_map::Entry::Occupied(entry) => {
                    return Err(PyValueError::new_err(format!(
                        "Duplicate constraint ID after integer promotion: {}",
                        entry.key()
                    )));
                }
            }
        }
        Ok(Self(result))
    }
}

impl<K, V> IntoIterator for ConstraintMapInput<K, V> {
    type Item = (K, V);
    type IntoIter = btree_map::IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for ConstraintMapInput<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<'py, K: IntoPyObject<'py> + Eq, V: IntoPyObject<'py>> IntoPyObject<'py>
    for ConstraintMapInput<K, V>
{
    type Target = PyDict;
    type Output = Bound<'py, PyDict>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        self.0.into_pyobject(py)
    }
}

impl<K: ConstraintIDInputType, V: PyStubType> PyStubType for ConstraintMapInput<K, V> {
    fn type_input() -> TypeInfo {
        BTreeMap::<K::ID, V>::type_input()
            | BTreeMap::<u64, V>::type_input()
            | BTreeMap::<K, V>::type_input()
    }

    fn type_output() -> TypeInfo {
        BTreeMap::<K::ID, V>::type_output()
    }
}

/// Input normalization for an API whose argument already fixes the family.
/// Keep this separate from the ID pyclass so heterogeneous ID inputs remain
/// strict and Python equality never promotes integers to IDs.
macro_rules! constraint_id_input {
    ($input:ident, $id:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) struct $input(ommx::$id);

        impl ConstraintIDInputType for $input {
            type ID = $id;
        }

        impl<'py> FromPyObject<'_, 'py> for $input {
            type Error = PyErr;

            fn extract(value: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
                if let Ok(id) = value.extract::<PyRef<$id>>() {
                    return Ok(Self(id.0));
                }
                if value.is_instance_of::<PyInt>() && !value.is_instance_of::<PyBool>() {
                    return Ok(Self(value.extract::<u64>()?.into()));
                }
                Err(PyTypeError::new_err(format!(
                    "Expected int or {}, got {}",
                    stringify!($id),
                    value.get_type().name()?
                )))
            }
        }

        impl From<$input> for ommx::$id {
            fn from(id: $input) -> Self {
                id.0
            }
        }

        impl From<ommx::$id> for $input {
            fn from(id: ommx::$id) -> Self {
                Self(id)
            }
        }

        impl<'py> IntoPyObject<'py> for $input {
            type Target = $id;
            type Output = Bound<'py, $id>;
            type Error = PyErr;

            fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
                $id::from(self.0).into_pyobject(py)
            }
        }

        impl std::fmt::Display for $input {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                $id::from(self.0).fmt(f)
            }
        }

        impl pyo3_stub_gen::PyStubType for $input {
            fn type_input() -> pyo3_stub_gen::TypeInfo {
                $id::type_input() | u64::type_input()
            }

            fn type_output() -> pyo3_stub_gen::TypeInfo {
                $id::type_output()
            }
        }
    };
}

constraint_id_input!(ConstraintIDInput, ConstraintID);
constraint_id_input!(IndicatorConstraintIDInput, IndicatorConstraintID);
constraint_id_input!(OneHotConstraintIDInput, OneHotConstraintID);
constraint_id_input!(Sos1ConstraintIDInput, Sos1ConstraintID);

macro_rules! constraint_id {
    ($name:ident, $doc:literal) => {
        // Pickle resolves the public module; stubs stay with the extension's
        // definitions and are re-exported from ommx as before.
        #[doc = $doc]
        #[pyo3_stub_gen::derive::gen_stub_pyclass(module = "ommx._ommx_rust")]
        #[pyclass(module = "ommx", frozen, eq, ord, hash, extends = ConstraintIDBase)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(ommx::$name);
        crate::constraint_base::constraint_subclass!(
            $name,
            crate::constraint_base::ConstraintIDBase
        );

        impl From<ommx::$name> for $name {
            fn from(id: ommx::$name) -> Self {
                Self(id)
            }
        }

        impl From<$name> for ommx::$name {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0.into_inner())
            }
        }

        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $name {
            /// Create a typed ID identifying this constraint family and number.
            ///
            /// The value must be an unsigned 64-bit integer. An ID of another
            /// family is not accepted. IDs have no integer conversion protocol.
            #[new]
            fn new(value: u64) -> Self {
                Self(value.into())
            }

            fn __repr__(&self) -> String {
                format!("{}({})", stringify!($name), self.0.into_inner())
            }

            /// Reconstruct the same ID family through its public constructor.
            /// The numeric payload is used only by the serialization protocol.
            fn __reduce__<'py>(&self, py: Python<'py>) -> (Bound<'py, PyType>, (u64,)) {
                (py.get_type::<Self>(), (self.0.into_inner(),))
            }

            /// IDs are immutable; copying preserves the same typed identity.
            fn __copy__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
                slf
            }

            /// IDs are immutable, including when nested in copied containers.
            fn __deepcopy__<'py>(
                slf: Bound<'py, Self>,
                _memo: &Bound<'py, PyAny>,
            ) -> Bound<'py, Self> {
                slf
            }
        }
    };
}

constraint_id!(
    ConstraintID,
    "A regular constraint ID. Unequal to integers and IDs of other constraint families.\n\nSee {class}`~ommx.ConstraintIDBase` for lookup, copying, serialization, and extension contracts."
);
constraint_id!(
    IndicatorConstraintID,
    "An indicator constraint ID. Unequal to integers and IDs of other constraint families.\n\nSee {class}`~ommx.ConstraintIDBase` for lookup, copying, serialization, and extension contracts."
);
constraint_id!(
    OneHotConstraintID,
    "A one-hot constraint ID. Unequal to integers and IDs of other constraint families.\n\nSee {class}`~ommx.ConstraintIDBase` for lookup, copying, serialization, and extension contracts."
);
constraint_id!(
    Sos1ConstraintID,
    "An SOS1 constraint ID. Unequal to integers and IDs of other constraint families.\n\nSee {class}`~ommx.ConstraintIDBase` for lookup, copying, serialization, and extension contracts."
);

/// An identity whose concrete Python type determines its constraint family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, IntoPyObject)]
pub enum AnyConstraintID {
    Regular(ConstraintID),
    Indicator(IndicatorConstraintID),
    OneHot(OneHotConstraintID),
    Sos1(Sos1ConstraintID),
}

impl<'py> FromPyObject<'_, 'py> for AnyConstraintID {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        macro_rules! try_id {
            ($id:ident, $variant:ident) => {
                if let Ok(id) = value.extract::<PyRef<$id>>() {
                    return Ok(Self::$variant(*id));
                }
            };
        }
        try_id!(ConstraintID, Regular);
        try_id!(IndicatorConstraintID, Indicator);
        try_id!(OneHotConstraintID, OneHot);
        try_id!(Sos1ConstraintID, Sos1);

        let name = value.get_type().name()?;
        if value.is_instance_of::<ConstraintIDBase>() {
            Err(PyTypeError::new_err(format!(
                "Unsupported constraint ID type: {name}"
            )))
        } else {
            Err(PyTypeError::new_err(format!(
                "Expected a typed constraint ID, got {name}. Use ConstraintID(n) for a regular constraint."
            )))
        }
    }
}

impl pyo3_stub_gen::PyStubType for AnyConstraintID {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        crate::constraint_base::ConstraintIDBase::type_output()
    }
}

macro_rules! any_constraint_id {
    ($id:ident, $variant:ident) => {
        impl From<ommx::$id> for AnyConstraintID {
            fn from(id: ommx::$id) -> Self {
                Self::$variant(id.into())
            }
        }
    };
}
any_constraint_id!(ConstraintID, Regular);
any_constraint_id!(IndicatorConstraintID, Indicator);
any_constraint_id!(OneHotConstraintID, OneHot);
any_constraint_id!(Sos1ConstraintID, Sos1);
