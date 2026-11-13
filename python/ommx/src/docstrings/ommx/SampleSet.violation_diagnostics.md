Explain violated constraints for one sample using the saved tolerance.

Returns the same read-only {class}`~ommx.ViolationDiagnostics` Mapping as
{meth}`~ommx.Solution.violation_diagnostics`, with the same role filtering and
snapshot semantics. Only the requested sample is inspected; a whole Solution
is not extracted. Unknown sample IDs raise `KeyError`, including when there
are no constraints. Variable and sample IDs are integers; diagnostic Mapping
keys are typed constraint IDs.
