Solve an Instance with an OMMX SolverAdapter and log a Solve entry.

The input Instance is cloned before calling the Adapter's easy
``solve()`` API. That API may prepare another isolated copy with the
Adapter's recommended Policy, so neither step mutates the caller's
object. The original caller input, before any recommended Preparation,
is always stored as the Solve input.

`adapter` must be a subclass of `ommx.adapter.SolverAdapter`. Keyword
arguments are passed to `adapter.solve(...)` and recorded as
`Solve.adapter_options`. The adapter class name is stored in
`Solve.adapter`.

Adapter options are solve-scoped metadata, not run parameters. They do
not appear in `Experiment.run_parameters_df()`.

Adapter diagnostics are disabled by default. Set
`store_diagnostics=True` to pass a diagnostics sink to the adapter and
store recorded diagnostics with the Solve entry. Diagnostics persistence
is best-effort. If diagnostics cannot be serialized or stored after the
adapter returns a solution, the Solve entry is still recorded without
diagnostics. `store_diagnostics` controls Experiment logging and is not
recorded in `Solve.adapter_options`.

If the adapter raises before returning a Solution, this method records
a failed Solve entry when possible, including diagnostics collected
before the failure when `store_diagnostics=True`. Failed Solve entries
have `output=None`.

# Record a parameter sweep and inspect the Solve records

Install the HiGHS Adapter for this example. It defines one knapsack problem
family, records two capacities, and reads both the comparison table and the
stored inputs/results. The Experiment's temporary Local Registry avoids
modifying existing records.

```pycon
>>> from ommx import DecisionVariable, Parameter, ParametricInstance, Sense
>>> from ommx.experiment import Experiment
>>> from ommx_highs_adapter import OMMXHighsAdapter
>>> values = [10, 13, 18, 31, 7, 15]
>>> weights = [11, 25, 20, 35, 10, 33]
>>> x = [DecisionVariable.binary(id=i, name="x", subscripts=[i]) for i in range(6)]
>>> capacity = Parameter(6, name="capacity")
>>> family = ParametricInstance.from_components(
...     decision_variables=x,
...     parameters=[capacity],
...     objective=sum(value * variable for value, variable in zip(values, x)),
...     constraints={
...         0: sum(weight * variable for weight, variable in zip(weights, x)) <= capacity
...     },
...     sense=Sense.Maximize,
... )
>>> with Experiment.with_temp_local_registry() as experiment:
...     experiment.log_parametric_instance("problem-family", family)
...     experiment.log_json("source-data", {"values": values, "weights": weights})
...     for limit in [47, 56]:
...         instance = family.with_parameters({capacity.id: limit})
...         with experiment.run() as run:
...             run.log_parameter("capacity", limit)
...             solution = run.log_solve(OMMXHighsAdapter, instance, verbose=False)
...             assert solution.feasible
...             run.log_parameter("objective", solution.objective)
>>> experiment.run_parameters_df()[["capacity", "objective"]].to_dict("records")
[{'capacity': 47, 'objective': 41.0}, {'capacity': 56, 'objective': 49.0}]
>>> for run in experiment.runs:
...     assert len(run.solves) == 1
...     solve = run.solves[0]
...     assert solve.solve_id == 0
...     assert solve.adapter.endswith("OMMXHighsAdapter")
...     assert solve.adapter_options == {"verbose": False}
...     assert solve.output is not None
...     assert solve.input.evaluate(solve.output.state).objective == solve.output.objective

```

The supplied input and options are retained, but an Adapter's internal
Preparation history or backend options changed outside this call are not
comprehensively collected. Log the source model, formulation choices, or other
conditions as separate attachments when they are needed later. Use
{class}`Experiment` for archive export and fresh-process reading, and
{meth}`Experiment.fork` to add trials without changing the parent record.
