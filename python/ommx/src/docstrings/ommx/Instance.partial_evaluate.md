Create a new Instance by fixing decision variables from a supplied state.

The original Instance remains unchanged. The new Instance validates and
canonicalizes supplied values, fixes the specified variables, and substitutes
them into active expressions. Use this to study a configuration or optimize
the remaining variables under chosen conditions.

`state` accepts a {class}`~ommx.State`, an ID-to-value mapping, or an iterable of
`(variable_id, value)` pairs. `atol` is the absolute tolerance; `None` uses
the default from {func}`~ommx.get_default_atol`.

**A fixed subproblem is not necessarily equivalent to the original**

For a binary variable, minimizing `x` has optimum zero, while fixing `x=1`
leaves a subproblem whose optimum is one. Evaluation can restore that fixed
value without establishing optimality for the original problem:

```pycon
>>> from ommx import Instance
>>> source = Instance.minimize()
>>> x = source.new_binary("x")
>>> source.objective = x
>>> original_candidate = source.evaluate({x.id: 0})
>>> conditioned = source.partial_evaluate({x.id: 1})
>>> conditioned_candidate = conditioned.evaluate({})
>>> assert original_candidate.feasible and original_candidate.objective == 0.0
>>> assert conditioned_candidate.feasible and conditioned_candidate.objective == 1.0
>>> assert conditioned_candidate.state.entries[x.id] == 1.0
>>> assert {v.id for v in source.used_decision_variables} == {x.id}

```

The optimum claims here follow by comparing the two binary values, not from
`evaluate`. If the original constraints already force the fixed value, fixing
it may instead be a simplification that preserves all feasible candidates.
This method does not prove that the remaining constraints are feasible; evaluate
an actual candidate to check them.

**Canonicalization and retained output values**

After kind and bound validation, accepted supplied coordinates follow the
Instance-owned rules in {meth}`~ommx.Instance.populate_state` before propagation
and substitution. Non-fixed, non-dependent Continuous and SemiContinuous values
are not rounded. Existing fixed values remain authoritative, and dependent
inputs remain consistency assertions. Values outside the partial-evaluation
acceptance contract remain rejected. The new Instance rewrites only active
expressions while retaining canonical fixed values for output evaluation.

```pycon
>>> from ommx import DecisionVariable, Instance, Sense
>>> x = DecisionVariable.binary(0)
>>> instance = Instance.from_components(
...     decision_variables=[x], objective=3 * x, constraints={}, sense=Sense.Maximize
... )
>>> assert instance.convert_active_objective(Sense.Minimize)
>>> fixed = instance.partial_evaluate({0: 1.0000005}, atol=1e-6)
>>> assert instance.required_ids() == {0}
>>> assert fixed.required_ids() == set()
>>> assert fixed.objective.evaluate({}) == -3.0
>>> assert fixed.attached_decision_variable(0).substituted_value == 1.0
>>> solution = fixed.evaluate({})
>>> assert (solution.sense, solution.objective) == (Sense.Maximize, 3.0)

```
