Create a decision variable whose feasible values are explicitly enumerated.

```pycon
>>> from ommx import DecisionVariable, Instance, Kind
>>> dose = DecisionVariable.finite_domain(0, [1.0, 0.1, 0.5, 0.3], name="dose")
>>> assert dose.kind == Kind.FiniteDomain
>>> dose.values
[0.1, 0.3, 0.5, 1.0]
>>> assert (dose.bound.lower, dose.bound.upper) == (0.1, 1.0)

```

`values` is the exact feasible set. It must be non-empty and contain unique,
finite numbers; OMMX stores the values in ascending order. Values between the
minimum and maximum that are not listed remain infeasible. This domain does
not approximate a continuous interval.

The {attr}`~ommx.DecisionVariable.bound` property and the `lower` and `upper`
columns in {meth}`~ommx.Instance.decision_variables_df` are derived from the
minimum and maximum of `values`. They do not independently define the domain.
The DataFrame's `values` column contains the exact set for finite-domain
variables and a missing value for interval-domain kinds.

For incremental construction with automatically assigned IDs, use
{meth}`~ommx.Instance.new_finite_domain`:

```pycon
>>> instance = Instance.minimize()
>>> dose = instance.new_finite_domain([0.1, 0.3, 0.5, 1.0], "dose")
>>> instance.objective = dose
>>> assert instance.evaluate({dose.id: 0.3}).feasible
>>> assert not instance.evaluate({dose.id: 0.4}).feasible

```
