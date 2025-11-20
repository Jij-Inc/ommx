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
            // Option overhead + delegated value
            visitor.visit_leaf(
                &path.with("Option[overhead]"),
                size_of::<Option<T>>() - size_of::<T>(), // size_of::<T> will be counted in the value
            );
            value.visit_logical_memory(path, visitor);
        } else {
            // Empty Option only has overhead
            visitor.visit_leaf(path, size_of::<Option<T>>());
        }
    }
}

impl<K, V> LogicalMemoryProfile for std::collections::BTreeMap<K, V>
where
    K: LogicalMemoryProfile,
    V: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // BTreeMap struct overhead
        let map_overhead = size_of::<std::collections::BTreeMap<K, V>>();
        visitor.visit_leaf(&path.with("BTreeMap[overhead]"), map_overhead);

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
        // HashMap struct overhead
        let map_overhead = size_of::<std::collections::HashMap<K, V>>();
        visitor.visit_leaf(&path.with("HashMap[overhead]"), map_overhead);

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

impl<K, V> LogicalMemoryProfile for fnv::FnvHashMap<K, V>
where
    K: LogicalMemoryProfile,
    V: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // FnvHashMap struct overhead
        let map_overhead = size_of::<fnv::FnvHashMap<K, V>>();
        visitor.visit_leaf(&path.with("FnvHashMap[overhead]"), map_overhead);

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

impl<T> LogicalMemoryProfile for Vec<T>
where
    T: LogicalMemoryProfile,
{
    fn visit_logical_memory<Vis: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut Vis) {
        // Vec struct overhead
        let vec_overhead = size_of::<Vec<T>>();
        visitor.visit_leaf(&path.with("Vec[overhead]"), vec_overhead);

        // Delegate to each element
        for element in self {
            element.visit_logical_memory(path, visitor);
        }
    }
}
