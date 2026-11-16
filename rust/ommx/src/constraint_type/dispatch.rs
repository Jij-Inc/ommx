//! Internal, typed dispatch over the SDK's constraint families.
//!
//! Result owners implement `Query` to keep collection access and value lookup
//! in their own modules. Dispatch only selects a type; it does not access rows.
//! These traits are unreachable outside the crate. `Sealed` is the private
//! supertrait of `ConstraintType`, so external generic callers need no host bound.

use crate::ConstraintType;

pub trait Sealed: Sized {
    fn dispatch<Q: Query>(query: Q) -> Q::Output<Self>
    where
        Self: ConstraintType;
}

macro_rules! define_dispatch {
    ((); $( $method:ident: $family:ty {
        created: $created:ident, evaluated: $evaluated:ident, sampled: $sampled:ident,
    } )*) => {
        pub trait Query {
            type Output<C: ConstraintType>;
            $(fn $method(self) -> Self::Output<$family>;)*
        }

        $(impl Sealed for $family {
            fn dispatch<Q: Query>(query: Q) -> Q::Output<Self> {
                query.$method()
            }
        })*
    };
}

super::families::with_constraint_families!(define_dispatch; ());
