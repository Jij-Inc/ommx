Read-only snapshot of violated constraints, keyed by typed constraint ID.

Obtain this Mapping from {meth}`~ommx.Solution.violation_diagnostics` or
{meth}`~ommx.SampleSet.violation_diagnostics`. It contains only constraints that
fail at the result's saved tolerance within the selected role scope. An empty
Mapping does not establish variable-domain feasibility.

Index with a typed ID to obtain that family's concrete diagnostic type. Use
`of_kind(ConstraintKind.OneHot)` (or another family) for a view accepting integer
keys. Mixed-family lookups reject integers; family views reject IDs from other
families. Missing keys raise `KeyError`, and `get` preserves a supplied default.

Iteration, `keys()`, `values()`, `items()`, and equality follow the Mapping
protocol. Iteration is ordered by family, then numeric ID. The Mapping and its
family views share an immutable diagnostic snapshot; they cannot modify the
owning result. Dictionaries and sets returned by diagnostic properties are
independent copies.

Values inherit {class}`~ommx.ViolationDiagnosticsBase`. Future SDKs may add
constraint families; consumers should inspect the concrete diagnostic type and
handle or reject unfamiliar types. Regular and Indicator diagnostics currently
carry no extra fields. SOS1 reports excess signed member values. OneHot separates
missing selection, multiple selections, and value errors, retaining zeros too.
