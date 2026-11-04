//! Open Python interfaces for constraint families. Domain state remains on the
//! owning roots; these bases neither store rows nor grant mutation authority.

use pyo3::{exceptions::PyNotImplementedError, prelude::*};
use std::collections::HashMap;

/// Common base of constraint IDs, including families introduced by future SDKs.
///
/// Consumers must handle the concrete ID types they support and reject all
/// others. This base alone is not a usable constraint identity. Integers are
/// not instances of this class and are never accepted by mixed-family lookups.
///
/// **Typed identity and integer inputs**
///
/// The currently supported concrete types are {class}`~ommx.ConstraintID`,
/// {class}`~ommx.IndicatorConstraintID`, {class}`~ommx.OneHotConstraintID`, and
/// {class}`~ommx.Sos1ConstraintID`. An ID carries its family and an unsigned
/// 64-bit number. IDs from different families are unequal, even if their
/// numbers coincide. Within one Instance, constraint numbers must still be
/// unique across all active and removed families.
///
/// APIs whose argument already fixes a family promote integer inputs to that
/// family's ID: for example, ``constraints={1: constraint}`` in
/// {meth}`~ommx.Instance.from_components`, or
/// ``instance.constraints.of_kind(ConstraintKind.Regular)[1]``. They reject IDs of another
/// family. Such mapping inputs reject ``1`` and ``ConstraintID(1)`` together
/// with ``ValueError``, because the keys normalize to the same identity.
/// Mixed-family collections and ``constraint_violation(id)`` require typed
/// IDs. DataFrame indexes also retain typed IDs. Constraint IDs do not support
/// ``int()`` or integer-index conversion; variable and sample IDs remain integers.
///
/// IDs are immutable. Shallow and deep copies return the same ID object;
/// copying containers or pickling preserves ID family and value, including
/// IDs used in DataFrame indexes or columns.
///
/// >>> import copy
/// >>> import pickle
/// >>> from ommx import ConstraintKind, ConstraintID, OneHotConstraintID
/// >>> cid = ConstraintID(7)
/// >>> assert cid != 7 and cid != OneHotConstraintID(7)
/// >>> assert copy.copy(cid) is cid
/// >>> assert copy.deepcopy(cid) is cid
/// >>> assert copy.deepcopy({cid: [cid]}) == {cid: [cid]}
/// >>> restored = pickle.loads(pickle.dumps({cid: [cid]}))
/// >>> assert restored == {cid: [cid]}
/// >>> assert type(next(iter(restored))) is ConstraintID
///
/// **Supporting additional families**
///
/// New special-constraint families are additive API extensions. Collections,
/// including removed and result collections, expose the open common interfaces
/// ``Mapping[ConstraintIDBase, ConstraintView]``; their known concrete ID
/// overloads retain precise row types. The currently supported families are
/// not an exhaustive list of future possibilities.
///
/// A solver adapter or transformation must reject families it cannot handle.
/// Silently skipping their constraints can relax the optimization problem.
/// Keep this check even if an earlier applicability check normally excludes
/// unsupported kinds. For example, a consumer requiring regular rows can
/// validate the complete collection before processing it:
///
/// >>> from ommx import ConstraintKind, ConstraintID, Instance, OneHotConstraint
/// >>> def regular_rows(model):
/// ...     rows = []
/// ...     for cid in model.constraints:
/// ...         if not isinstance(cid, ConstraintID):
/// ...             raise NotImplementedError("Unsupported constraint family")
/// ...         rows.append(model.constraints[cid])
/// ...     return rows
/// >>> model = Instance.minimize()
/// >>> x = model.new_binary("x")
/// >>> y = model.new_binary("y")
/// >>> rule = model.add_constraint(x + y <= 1)
/// >>> assert regular_rows(model) == [rule]
/// >>> _ = model.add_one_hot_constraint(OneHotConstraint(variables=[x, y]))
/// >>> regular_rows(model)
/// Traceback (most recent call last):
/// ...
/// NotImplementedError: Unsupported constraint family
///
/// Handle additional concrete ID types explicitly when supported and retain
/// the rejecting fallback. Receiving a new family in an older consumer should
/// fail explicitly. Metadata-only inspection can use {class}`~ommx.ConstraintView`
/// without interpreting each family's mathematical meaning.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(subclass, frozen)]
#[derive(Default)]
pub struct ConstraintIDBase;

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintIDBase {
    #[new]
    fn new() -> Self {
        Self
    }
}

/// Common metadata interface for rows fetched from constraint collections.
///
/// The set of concrete constraint families is open. Inspect a concrete ID or
/// row type before using family-specific mathematical operations, and reject
/// unsupported families rather than dropping their constraints. This interface
/// does not imply whether the row is attached or an owned result snapshot.
/// See {class}`~ommx.ConstraintCollection` for the ownership and iteration
/// contracts and {class}`~ommx.ConstraintIDBase` for explicit family dispatch.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(subclass)]
#[derive(Default)]
pub struct ConstraintView;

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ConstraintView {
    #[getter]
    fn name(&self) -> PyResult<Option<String>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement name",
        ))
    }
    #[getter]
    fn description(&self) -> PyResult<Option<String>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement description",
        ))
    }
    #[getter]
    fn subscripts(&self) -> PyResult<Vec<i64>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement subscripts",
        ))
    }
    #[getter]
    fn parameters(&self) -> PyResult<HashMap<String, String>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement parameters",
        ))
    }
    #[getter]
    fn provenance(&self) -> PyResult<Vec<crate::Provenance>> {
        Err(PyNotImplementedError::new_err(
            "Constraint family must implement provenance",
        ))
    }
}

// Every binding family uses this initializer to preserve its concrete Python
// class while adding the common base. No domain data is moved into the base.
macro_rules! constraint_subclass {
    ($class:ty, $base:ty) => {
        impl From<$class> for pyo3::PyClassInitializer<$class> {
            fn from(value: $class) -> Self {
                pyo3::PyClassInitializer::from(<$base>::default()).add_subclass(value)
            }
        }
        impl<'py> pyo3::IntoPyObject<'py> for $class {
            type Target = Self;
            type Output = pyo3::Bound<'py, Self>;
            type Error = pyo3::PyErr;
            fn into_pyobject(self, py: pyo3::Python<'py>) -> pyo3::PyResult<Self::Output> {
                pyo3::Bound::new(py, self)
            }
        }
    };
}
// Shared only with the ID and row binding modules implementing these bases.
pub(crate) use constraint_subclass;
