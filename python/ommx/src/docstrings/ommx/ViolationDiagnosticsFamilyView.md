Read-only view of one diagnostic family.

Created by `ViolationDiagnostics.of_kind(ConstraintKind.…)`. Keys retain their
typed identity; indexing and `get` also accept integers, promoted to the selected
family. IDs from other families are rejected. Missing keys raise `KeyError`,
and `get` returns the supplied default or `None`.

The view shares its immutable diagnostic snapshot. It supports iteration,
`keys()`, `values()`, `items()`, and Mapping equality. The key and value type
parameters preserve the selected family's concrete types for static checking.
