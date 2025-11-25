//! Path management for logical memory profiling.

/// Logical path for memory profiling.
///
/// A wrapper around `Vec<&'static str>` that provides a clean API for
/// path management in logical memory profiling.
#[derive(Debug, Default)]
pub struct Path(Vec<&'static str>);

impl Path {
    /// Create a new empty path.
    pub fn new() -> Self {
        Self(vec![])
    }

    /// Get the path as a slice.
    pub fn as_slice(&self) -> &[&'static str] {
        &self.0
    }

    /// Create a path guard that automatically pops on drop.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ommx::logical_memory::{Path, LogicalMemoryVisitor};
    /// use std::mem::size_of;
    ///
    /// # struct MyVisitor;
    /// # impl ommx::logical_memory::LogicalMemoryVisitor for MyVisitor {
    /// #     fn visit_leaf(&mut self, _path: &Path, _bytes: usize) {}
    /// # }
    /// let mut path = Path::new();
    /// let mut visitor = MyVisitor;
    ///
    /// visitor.visit_leaf(&path.with("field"), size_of::<u64>());
    /// // path is automatically popped when guard is dropped
    /// ```
    pub fn with(&mut self, name: &'static str) -> PathGuard<'_> {
        PathGuard::new(self, name)
    }
}

impl From<Vec<&'static str>> for Path {
    fn from(segments: Vec<&'static str>) -> Self {
        Self(segments)
    }
}

/// RAII guard for path management that automatically pops on drop.
///
/// This guard ensures that path push/pop operations are always paired,
/// preventing bugs from forgetting to pop.
///
/// # Example
///
/// ```rust
/// use ommx::logical_memory::{Path, LogicalMemoryVisitor};
/// use std::mem::size_of;
///
/// # struct MyVisitor;
/// # impl ommx::logical_memory::LogicalMemoryVisitor for MyVisitor {
/// #     fn visit_leaf(&mut self, _path: &Path, _bytes: usize) {}
/// # }
/// let mut path = Path::new();
/// let mut visitor = MyVisitor;
///
/// // Automatic pop via guard:
/// visitor.visit_leaf(&path.with("field"), size_of::<u64>());
/// // path is automatically popped when guard is dropped
/// ```
pub struct PathGuard<'a> {
    path: &'a mut Path,
}

impl<'a> PathGuard<'a> {
    /// Create a new path guard by pushing a name onto the path.
    fn new(path: &'a mut Path, name: &'static str) -> Self {
        path.0.push(name);
        Self { path }
    }

    /// Create a nested path guard.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ommx::logical_memory::{Path, LogicalMemoryVisitor};
    /// # struct MyVisitor;
    /// # impl ommx::logical_memory::LogicalMemoryVisitor for MyVisitor {
    /// #     fn visit_leaf(&mut self, _path: &Path, _bytes: usize) {}
    /// # }
    /// let mut path = Path::new();
    /// let mut visitor = MyVisitor;
    ///
    /// // Nested guards
    /// let mut guard1 = path.with("parent");
    /// visitor.visit_leaf(&guard1.with("child"), 42);
    /// // Both "child" and "parent" are automatically popped in reverse order
    /// ```
    pub fn with(&mut self, name: &'static str) -> PathGuard<'_> {
        PathGuard::new(self.path, name)
    }
}

impl Drop for PathGuard<'_> {
    fn drop(&mut self) {
        self.path.0.pop();
    }
}

impl std::ops::Deref for PathGuard<'_> {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path
    }
}

impl AsRef<Path> for PathGuard<'_> {
    fn as_ref(&self) -> &Path {
        self.path
    }
}

impl AsMut<Path> for PathGuard<'_> {
    fn as_mut(&mut self) -> &mut Path {
        self.path
    }
}
