A labeled expression for reporting a candidate's additional metrics.

For example, minimize cost while also recording distance or workload. Register
the expression in an {class}`~ommx.Instance`, evaluate a candidate, and retrieve
its value from the {class}`~ommx.Solution`. Registering a NamedFunction does not
make it an objective or a constraint.

**Register, evaluate, and read a metric**

Choose exactly one of two alternatives. The objective is cost; ``distance`` is
an additional measurement of the same candidate.

```pycon
>>> from ommx import DecisionVariable, Instance, NamedFunction, Sense
>>> x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
>>> distance = NamedFunction(id=0, function=10 * x[0] + 3 * x[1], name="distance")
>>> instance = Instance.from_components(
...     sense=Sense.Minimize,
...     objective=x[0] + 2 * x[1],
...     decision_variables=x,
...     constraints={0: sum(x) == 1},
...     named_functions=[distance],
... )
>>> solution = instance.evaluate({x[0].id: 1, x[1].id: 0})
>>> assert solution.feasible
>>> assert solution.objective == 1.0
>>> assert solution.extract_named_functions("distance") == {(): 10.0}
>>> metrics = solution.named_functions_df()
>>> assert metrics.loc[distance.id, "name"] == "distance"
>>> assert metrics.loc[distance.id, "value"] == 10.0

```

This candidate costs 1 and has distance 10. The distance did not contribute to
the objective or the feasibility conditions. All variables referenced by the
metric must still be declared in the Instance. Named-function IDs form their own
namespace, separate from variable and constraint IDs; they must be unique within
the Instance. An {class}`~ommx.ParametricInstance` can also hold named functions
whose expressions contain its declared parameters.

Use {meth}`~ommx.Solution.extract_named_functions` for a dictionary keyed by
subscript tuples, or {meth}`~ommx.Solution.named_functions_df` for a table indexed
by named-function ID. A scalar metric has no subscripts and uses ``()`` as its
dictionary key. To label a metric for each facility, for example, reuse a name
with distinct ``subscripts``. Names and subscripts are modeling labels;
``parameters`` and ``description`` do not disambiguate duplicate extraction keys.
The extraction methods specify missing-name and duplicate-key errors.

**Compare metrics across candidates**

{meth}`~ommx.Instance.evaluate_samples` evaluates the registered metrics for each
candidate. Select one using its sample ID when querying a
{class}`~ommx.SampleSet`.

```pycon
>>> results = instance.evaluate_samples(
...     {
...         7: {x[0].id: 1, x[1].id: 0},
...         9: {x[0].id: 0, x[1].id: 1},
...     }
... )
>>> assert results.extract_named_functions("distance", 7) == {(): 10.0}
>>> assert results.extract_named_functions("distance", 9) == {(): 3.0}
>>> assert results.best_feasible.objective == 1.0
>>> assert results.best_feasible.extract_named_functions("distance") == {(): 10.0}

```

The cheapest candidate is not the one with the shortest distance. Registering
metrics does not change the objective-based ordering or the selection made by
{attr}`~ommx.SampleSet.best_feasible`.

**Supply values for variables used only in reporting**

Appearing only in a named function does not make a variable used by the current
objective or active constraints. If it also has no fixed value or dependency,
the variable is irrelevant to that current problem. When its value is omitted,
{meth}`~ommx.Instance.populate_state` supplies a default before the named
function is evaluated. That default is not a recovered value of the candidate.

```pycon
>>> recorded = DecisionVariable.integer(0, lower=2, upper=5, name="recorded")
>>> reporting = Instance.from_components(
...     sense=Sense.Minimize,
...     objective=0,
...     decision_variables=[recorded],
...     constraints={},
...     named_functions=[NamedFunction(id=0, function=recorded, name="recorded")],
... )
>>> omitted = reporting.evaluate({})
>>> explicit = reporting.evaluate({recorded.id: 4})
>>> assert omitted.extract_named_functions("recorded") == {(): 2.0}
>>> assert explicit.extract_named_functions("recorded") == {(): 4.0}
>>> assert not reporting.evaluate({recorded.id: 6}).feasible

```

Pass the candidate's actual value when it matters to reporting. Variable kinds
and bounds are still checked by {meth}`~ommx.Instance.evaluate`, including for
variables that appear only in a metric. Calling {meth}`~ommx.NamedFunction.evaluate`
directly evaluates just this expression; it does not check an Instance's variable
domains or constraints or reconstruct its omitted values.

**Ownership**

An Instance stores the registered expression and its labels. Objects returned by
{attr}`~ommx.Instance.named_functions` or
{meth}`~ommx.Instance.get_named_function_by_id` are independent snapshots.
Partially evaluating a retrieved object does not replace the registered metric.

```pycon
>>> snapshot = instance.get_named_function_by_id(distance.id)
>>> _ = snapshot.partial_evaluate({x[0].id: 1, x[1].id: 0})
>>> assert snapshot.function.evaluate({}) == 10.0
>>> assert instance.evaluate({x[0].id: 0, x[1].id: 1}).extract_named_functions(
...     "distance"
... ) == {(): 3.0}

```

Arithmetic on a NamedFunction produces a {class}`~ommx.Function`; comparisons
such as ``metric <= limit`` construct a {class}`~ommx.Constraint`. These new
expressions or constraints take effect in a model only when assigned or added
through that model's APIs.
