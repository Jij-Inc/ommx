//! Read-only traversal expanded inside the root that owns the operation.
//!
//! Each block sees a concrete collection type. There is no type erasure or
//! mutable access. Builders and parsers use borrowed collections before a root
//! exists; adding a registry entry makes missing fields a compilation error.

macro_rules! define_borrowed_collections {
    ((); $( $method:ident: $family:ty {
        created: $created:ident, evaluated: $evaluated:ident, sampled: $sampled:ident,
    } )*) => {
        /// Model-owner construction paths borrow all created collections together.
        pub(crate) struct CreatedCollections<'a> {
            $(pub(crate) $created: &'a crate::ConstraintCollection<$family>,)*
        }

        /// Result-owner construction paths borrow all evaluated collections together.
        pub(crate) struct EvaluatedCollections<'a> {
            $(pub(crate) $evaluated: &'a crate::EvaluatedCollection<$family>,)*
        }

        /// Result-owner construction paths borrow all sampled collections together.
        pub(crate) struct SampledCollections<'a> {
            $(pub(crate) $sampled: &'a crate::SampledCollection<$family>,)*
        }
    };
}

crate::constraint_type::families::with_constraint_families!(define_borrowed_collections; ());

macro_rules! collection_field {
    (created, $owner:ident, $created:ident, $evaluated:ident, $sampled:ident) => {
        &$owner.$created
    };
    (evaluated, $owner:ident, $created:ident, $evaluated:ident, $sampled:ident) => {
        &$owner.$evaluated
    };
    (sampled, $owner:ident, $created:ident, $evaluated:ident, $sampled:ident) => {
        &$owner.$sampled
    };
}

macro_rules! visit_collections {
    (($stage:ident, $owner:expr, $kind:pat_param, $collection:ident, $body:block);
        $( $method:ident: $family:ty {
            created: $created:ident, evaluated: $evaluated:ident, sampled: $sampled:ident,
        } )*
    ) => {{
        let owner = &$owner;
        $({
            let $kind = <$family as $crate::ConstraintType>::KIND;
            let $collection = $crate::constraint_collection::traverse::collection_field!(
                $stage, owner, $created, $evaluated, $sampled
            );
            $body
        })*
    }};
}

/// Expand a block for each concrete family in registration order.
///
/// The owner can be a model/result root or a borrowed collection bundle.
/// `?` and `return` retain their meaning in the calling root operation.
macro_rules! for_each_constraint_collection {
    ($stage:ident, $owner:expr, |$kind:pat_param, $collection:ident| $body:block) => {
        $crate::constraint_type::families::with_constraint_families!(
            $crate::constraint_collection::traverse::visit_collections;
            ($stage, $owner, $kind, $collection, $body)
        )
    };
}

// Macros expand in Instance, Solution, and SampleSet to keep field access and
// domain decisions inside their owners. Helpers need paths for macro expansion.
pub(crate) use {collection_field, for_each_constraint_collection, visit_collections};
