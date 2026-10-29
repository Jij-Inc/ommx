Convert only the active, solver-facing objective to `target`.

For example, a solver may accept only minimization while the problem maximizes
a reward. This method negates the active objective and changes
{attr}`~ommx.Instance.sense`, preserving the objective and sense returned by
{meth}`~ommx.Instance.evaluate` and {meth}`~ommx.Instance.evaluate_samples`:

```pycon
>>> from ommx import Instance, Sense
>>> instance = Instance.maximize()
>>> reward = instance.new_binary("reward")
>>> instance.objective = 3 * reward
>>> assert instance.output_objective is None
>>> assert instance.convert_active_objective(Sense.Minimize)
>>> assert instance.sense == Sense.Minimize
>>> assert instance.objective.evaluate({reward.id: 1}) == -3.0
>>> reported = instance.evaluate({reward.id: 1})
>>> assert (reported.sense, reported.objective) == (Sense.Maximize, 3.0)
>>> assert not instance.convert_active_objective(Sense.Minimize)

```

Returns `True` if the active objective was converted and `False` if its sense
already equals `target`. On the first conversion without an output objective,
the previous active objective and sense are captured in
{attr}`~ommx.Instance.output_objective`. An existing output objective is kept.
Once captured, it remains explicit even if a later conversion makes it equal
to the active objective. Conversion works in either direction:

```pycon
>>> for source, target in (
...     (Sense.Maximize, Sense.Minimize),
...     (Sense.Minimize, Sense.Maximize),
... ):
...     model = Instance.minimize() if source == Sense.Minimize else Instance.maximize()
...     x = model.new_binary("x")
...     model.objective = 3 * x
...     before = model.evaluate({x.id: 1})
...     assert model.convert_active_objective(target)
...     after = model.evaluate({x.id: 1})
...     assert model.sense == target
...     assert model.objective.evaluate({x.id: 1}) == -3.0
...     assert (after.sense, after.objective) == (before.sense, before.objective)
...     assert not model.convert_active_objective(target)

```

Use {meth}`~ommx.Instance.as_minimization_problem` or
{meth}`~ommx.Instance.as_maximization_problem` when both the active and output
objective semantics should change. Assigning a new {attr}`~ommx.Instance.objective`
also rebases output evaluation onto that objective.

The reported objective is separate from feasibility and optimality. In
particular, after penalty conversion a small {attr}`~ommx.Solution.objective`
does not measure the penalty or establish that retained constraints are
satisfied. Check {attr}`~ommx.Solution.feasible` independently. Equivalent
objective sign conversion preserves optimality, but other transformations can
invalidate its transport; use {meth}`~ommx.Instance.map_active_optimality` for
the recorded mapping of a backend's optimality status.
