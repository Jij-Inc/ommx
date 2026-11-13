//! Internal, typed dispatch over the SDK's constraint families.
//!
//! Result owners implement `Query` to keep collection access and value lookup
//! in their own modules. Dispatch only selects a type; it does not access rows.
//! These traits are unreachable outside the crate. `Sealed` is the private
//! supertrait of `ConstraintType`, so external generic callers need no host bound.

use crate::{Constraint, ConstraintType, IndicatorConstraint, OneHotConstraint, Sos1Constraint};

pub trait Sealed: Sized {
    fn dispatch<Q: Query>(query: Q) -> Q::Output<Self>
    where
        Self: ConstraintType;
}

pub trait Query {
    type Output<C: ConstraintType>;

    fn regular(self) -> Self::Output<Constraint>;
    fn indicator(self) -> Self::Output<IndicatorConstraint>;
    fn one_hot(self) -> Self::Output<OneHotConstraint>;
    fn sos1(self) -> Self::Output<Sos1Constraint>;
}

macro_rules! impl_dispatch {
    ($family:ty, $method:ident) => {
        impl Sealed for $family {
            fn dispatch<Q: Query>(query: Q) -> Q::Output<Self> {
                query.$method()
            }
        }
    };
}

impl_dispatch!(Constraint, regular);
impl_dispatch!(IndicatorConstraint, indicator);
impl_dispatch!(OneHotConstraint, one_hot);
impl_dispatch!(Sos1Constraint, sos1);
