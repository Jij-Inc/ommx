//! Family-specific identities for native constraint-result dictionaries.
//!
//! These are not Python int subclasses: IDs from different constraint families
//! must remain unequal even when their numerical values coincide. Conversion to
//! raw integers is confined to internal serialization boundaries.

use pyo3::prelude::*;

macro_rules! constraint_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[pyo3_stub_gen::derive::gen_stub_pyclass]
        #[pyclass(frozen, eq, ord, hash)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(ommx::$name);

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
            /// Create an ID in this constraint family's namespace.
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
        }
    };
}

constraint_id!(
    ConstraintID,
    "A regular constraint ID. Unequal to integers and IDs of other constraint families."
);
constraint_id!(
    IndicatorConstraintID,
    "An indicator constraint ID. Unequal to integers and IDs of other constraint families."
);
constraint_id!(
    OneHotConstraintID,
    "A one-hot constraint ID. Unequal to integers and IDs of other constraint families."
);
constraint_id!(
    Sos1ConstraintID,
    "An SOS1 constraint ID. Unequal to integers and IDs of other constraint families."
);

/// An identity whose concrete Python type determines its constraint family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, FromPyObject, IntoPyObject)]
pub enum AnyConstraintID {
    Regular(ConstraintID),
    Indicator(IndicatorConstraintID),
    OneHot(OneHotConstraintID),
    Sos1(Sos1ConstraintID),
}

impl pyo3_stub_gen::PyStubType for AnyConstraintID {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        ConstraintID::type_output()
            | IndicatorConstraintID::type_output()
            | OneHotConstraintID::type_output()
            | Sos1ConstraintID::type_output()
    }
}

impl From<&ommx::Provenance> for AnyConstraintID {
    fn from(p: &ommx::Provenance) -> Self {
        match *p {
            ommx::Provenance::IndicatorConstraint(id) => Self::Indicator(id.into()),
            ommx::Provenance::OneHotConstraint(id) => Self::OneHot(id.into()),
            ommx::Provenance::Sos1Constraint(id) => Self::Sos1(id.into()),
        }
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
