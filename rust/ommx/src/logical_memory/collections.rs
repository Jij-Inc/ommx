//! LogicalMemoryProfile implementations for standard collections and common types.

use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use std::mem::size_of;

// Implementation for String
impl LogicalMemoryProfile for String {
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // String overhead + heap-allocated bytes
        let total_bytes = size_of::<String>() + self.len();
        visitor.visit_leaf(path, total_bytes);
    }
}

// Implementation for Option<T>
impl<T> LogicalMemoryProfile for Option<T>
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        if let Some(value) = self {
            // Option additional stack (Option size minus inner T size)
            visitor.visit_leaf(
                &path.with("Option[additional stack]"),
                size_of::<Option<T>>() - size_of::<T>(), // size_of::<T> will be counted in the value
            );
            value.visit_logical_memory(path, visitor);
        } else {
            // Empty Option only has stack
            visitor.visit_leaf(&path.with("Option[stack]"), size_of::<Option<T>>());
        }
    }
}

impl<T, const N: usize> LogicalMemoryProfile for [T; N]
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        for element in self {
            element.visit_logical_memory(path.with("Array[element]").as_mut(), visitor);
        }
    }
}

impl<T> LogicalMemoryProfile for Box<T>
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        visitor.visit_leaf(&path.with("Box[stack]"), size_of::<Box<T>>());
        self.as_ref()
            .visit_logical_memory(path.with("Box[value]").as_mut(), visitor);
    }
}

impl<T: ?Sized> LogicalMemoryProfile for std::marker::PhantomData<T> {
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        visitor.visit_leaf(path, size_of::<std::marker::PhantomData<T>>());
    }
}

impl<K, V> LogicalMemoryProfile for std::collections::BTreeMap<K, V>
where
    K: LogicalMemoryProfile,
    V: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // BTreeMap struct stack
        let map_stack = size_of::<std::collections::BTreeMap<K, V>>();
        visitor.visit_leaf(&path.with("BTreeMap[stack]"), map_stack);

        // Keys
        for k in self.keys() {
            k.visit_logical_memory(path.with("BTreeMap[key]").as_mut(), visitor);
        }

        // Delegate to each value
        for value in self.values() {
            value.visit_logical_memory(path, visitor);
        }
    }
}

impl<K, V> LogicalMemoryProfile for std::collections::HashMap<K, V>
where
    K: LogicalMemoryProfile,
    V: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // HashMap struct stack
        let map_stack = size_of::<std::collections::HashMap<K, V>>();
        visitor.visit_leaf(&path.with("HashMap[stack]"), map_stack);

        // Keys
        for k in self.keys() {
            k.visit_logical_memory(path.with("HashMap[key]").as_mut(), visitor);
        }

        // Delegate to each value
        for value in self.values() {
            value.visit_logical_memory(path, visitor);
        }
    }
}

impl<T> LogicalMemoryProfile for std::collections::HashSet<T>
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        let set_stack = size_of::<std::collections::HashSet<T>>();
        visitor.visit_leaf(&path.with("HashSet[stack]"), set_stack);

        for element in self {
            element.visit_logical_memory(path, visitor);
        }
    }
}

impl<K, V> LogicalMemoryProfile for fnv::FnvHashMap<K, V>
where
    K: LogicalMemoryProfile,
    V: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // FnvHashMap struct stack
        let map_stack = size_of::<fnv::FnvHashMap<K, V>>();
        visitor.visit_leaf(&path.with("FnvHashMap[stack]"), map_stack);

        // Keys
        for k in self.keys() {
            k.visit_logical_memory(path.with("FnvHashMap[key]").as_mut(), visitor);
        }

        // Delegate to each value
        for value in self.values() {
            value.visit_logical_memory(path, visitor);
        }
    }
}

impl<T> LogicalMemoryProfile for fnv::FnvHashSet<T>
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        let set_stack = size_of::<fnv::FnvHashSet<T>>();
        visitor.visit_leaf(&path.with("FnvHashSet[stack]"), set_stack);

        for element in self {
            element.visit_logical_memory(path, visitor);
        }
    }
}

impl<T> LogicalMemoryProfile for Vec<T>
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // Vec struct stack
        let vec_stack = size_of::<Vec<T>>();
        visitor.visit_leaf(&path.with("Vec[stack]"), vec_stack);

        // Delegate to each element
        for element in self {
            element.visit_logical_memory(path, visitor);
        }
    }
}

impl<T> LogicalMemoryProfile for std::collections::VecDeque<T>
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        let deque_stack = size_of::<std::collections::VecDeque<T>>();
        visitor.visit_leaf(&path.with("VecDeque[stack]"), deque_stack);

        for element in self {
            element.visit_logical_memory(path, visitor);
        }
    }
}

impl<T> LogicalMemoryProfile for std::collections::BTreeSet<T>
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // BTreeSet struct stack
        let set_stack = size_of::<std::collections::BTreeSet<T>>();
        visitor.visit_leaf(&path.with("BTreeSet[stack]"), set_stack);

        // Delegate to each element
        for element in self {
            element.visit_logical_memory(path, visitor);
        }
    }
}

macro_rules! impl_tuple_logical_memory_profile {
    ($(($index:tt, $name:ident)),+ $(,)?) => {
        impl<$($name),+> LogicalMemoryProfile for ($($name,)+)
        where
            $($name: LogicalMemoryProfile),+
        {
            fn visit_logical_memory<Vis: LogicalMemoryVisitor>(
                &self,
                path: &mut Path,
                visitor: &mut Vis,
            ) {
                $(
                    self.$index.visit_logical_memory(
                        path.with(concat!("Tuple.", stringify!($index))).as_mut(),
                        visitor,
                    );
                )+
            }
        }
    };
}

impl_tuple_logical_memory_profile!((0, T0));
impl_tuple_logical_memory_profile!((0, T0), (1, T1));
impl_tuple_logical_memory_profile!((0, T0), (1, T1), (2, T2));
impl_tuple_logical_memory_profile!((0, T0), (1, T1), (2, T2), (3, T3));
impl_tuple_logical_memory_profile!((0, T0), (1, T1), (2, T2), (3, T3), (4, T4));
impl_tuple_logical_memory_profile!((0, T0), (1, T1), (2, T2), (3, T3), (4, T4), (5, T5));
impl_tuple_logical_memory_profile!(
    (0, T0),
    (1, T1),
    (2, T2),
    (3, T3),
    (4, T4),
    (5, T5),
    (6, T6)
);
impl_tuple_logical_memory_profile!(
    (0, T0),
    (1, T1),
    (2, T2),
    (3, T3),
    (4, T4),
    (5, T5),
    (6, T6),
    (7, T7)
);
impl_tuple_logical_memory_profile!(
    (0, T0),
    (1, T1),
    (2, T2),
    (3, T3),
    (4, T4),
    (5, T5),
    (6, T6),
    (7, T7),
    (8, T8)
);
impl_tuple_logical_memory_profile!(
    (0, T0),
    (1, T1),
    (2, T2),
    (3, T3),
    (4, T4),
    (5, T5),
    (6, T6),
    (7, T7),
    (8, T8),
    (9, T9)
);
impl_tuple_logical_memory_profile!(
    (0, T0),
    (1, T1),
    (2, T2),
    (3, T3),
    (4, T4),
    (5, T5),
    (6, T6),
    (7, T7),
    (8, T8),
    (9, T9),
    (10, T10)
);
impl_tuple_logical_memory_profile!(
    (0, T0),
    (1, T1),
    (2, T2),
    (3, T3),
    (4, T4),
    (5, T5),
    (6, T6),
    (7, T7),
    (8, T8),
    (9, T9),
    (10, T10),
    (11, T11)
);
