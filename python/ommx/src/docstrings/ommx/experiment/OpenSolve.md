Context manager for a manually executed Solve inside a Run.

`OpenSolve` is returned by `Run.open_solve(...)`. Use `solver_input` to
access the backend solver model, run backend-specific APIs, decode the
solver output, and record adapter options that are set directly on the
backend model. The Solve entry is finalized when the context exits.

Most recording uses {meth}`Run.log_solve`. Use this scope when a backend API
must be called directly. Options set through the backend do not automatically
become stored metadata; record them with {meth}`log_adapter_option`.

This complete example requires the HiGHS Adapter:

```pycon
>>> from ommx import Instance
>>> from ommx.experiment import Experiment
>>> from ommx_highs_adapter import OMMXHighsAdapter
>>> problem = Instance.minimize()
>>> x = problem.new_binary("x")
>>> problem.objective = x
>>> with Experiment.with_temp_local_registry() as experiment:
...     with experiment.run() as run:
...         with run.open_solve(OMMXHighsAdapter, problem, verbose=False) as solve:
...             model = solve.solver_input
...             _ = model.setOptionValue("time_limit", 10.0)
...             solve.log_adapter_option("time_limit", 10.0)
...             _ = model.run()
...             solution = solve.decode(model)
...             assert solution.feasible
>>> recorded = experiment.runs[0].solves[0]
>>> recorded.output.objective
0.0
>>> recorded.adapter_options["time_limit"]
10.0

```

{meth}`decode` stores the decoded Solution as the Solve's output. The context
manager finalizes the record on exit; the individual methods document terminal
states, diagnostics, tracing, and exceptional exits. A finished recording
scope is distinct from a guarantee of candidate feasibility or optimality.
