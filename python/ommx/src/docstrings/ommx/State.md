A candidate assignment from integer decision-variable IDs to numeric values.

A State contains values, not the variable domains, objective, or constraints of
an optimization problem. Creating one does not establish feasibility or
optimality. Pass it to {meth}`~ommx.Instance.evaluate` on the corresponding
problem to obtain a {class}`~ommx.Solution` with evaluation results.

>>> from ommx import Instance, State
>>> instance = Instance.maximize()
>>> x = instance.new_binary("x")
>>> instance.objective = x
>>> candidate = State({x.id: 1})
>>> assert candidate.entries == {x.id: 1.0}
>>> result = instance.evaluate(candidate)
>>> assert result.feasible and result.objective == 1.0

APIs accepting a State also accept an ID-to-value mapping or an iterable of
ID/value pairs. The assignment must use the IDs of the Instance being evaluated.
After transforming a problem, evaluate the returned candidate with that
transformed Instance so that its recorded fixed values, dependencies, retained
constraints, and output objective can be interpreted correctly.

The returned Solution's {attr}`~ommx.Solution.state` contains the populated
assignment. Missing required IDs are errors; reconstruction, default values for
irrelevant variables, and tolerance-based normalization are documented by
{meth}`~ommx.Instance.populate_state`.
