ommx.experiment
===============

``ommx.experiment`` records an optimization experiment as one OMMX
Artifact. Use it when a single source problem produces several runs, for
example different formulations, decomposition strategies, disabled
constraints, or solver settings, and you want the resulting Artifact to
answer:

- which shared dataset or baseline configuration was used,
- which run-level parameters should be compared as a table,
- which input :class:`~ommx.v1.Instance` was actually solved, and
- which output :class:`~ommx.v1.Solution` and solver kwargs came from each
  solver call.

Motivating Example
------------------

The central workflow is to create an :class:`Experiment`, attach
experiment-wide context, create one :class:`Run` per comparison condition,
and call :meth:`Run.log_solve <ommx.experiment.Run.log_solve>` for each
solver invocation. The whole experiment is committed as one immutable
:class:`~ommx.artifact.Artifact`.

.. code-block:: python

   from ommx.experiment import Experiment
   from ommx_highs_adapter import OMMXHighsAdapter

   with Experiment("example.com/team/knapsack:latest") as exp:
       exp.log_json("dataset", {"name": "knapsack-demo"})

       for capacity, instance in candidate_instances:
           with exp.run() as run:
               # Run parameters are scalar values for comparison tables.
               run.log_parameter("capacity", capacity)

               # Run attachments hold auxiliary payloads for this run.
               run.log_json("scenario", {"capacity": capacity})

               # log_solve executes the adapter and records a Solve entry.
               solution = run.log_solve(
                   OMMXHighsAdapter,
                   instance,
                   time_limit=10.0,
               )

   artifact = exp.artifact
   run_parameters = exp.run_parameters_df()

The committed experiment can be loaded back as a read-only view. A
:class:`Solve` stores descriptors for the input instance and output
solution; use the parent artifact to read the payloads.

.. code-block:: python

   loaded = Experiment.load("example.com/team/knapsack:latest")
   artifact = loaded.artifact

   first_solve = loaded.runs[0].solves[0]
   input_instance = artifact.get_instance(first_solve.input)
   output_solution = artifact.get_solution(first_solve.output)

   # Solver kwargs are solve-scoped metadata, not Run parameters.
   print(first_solve.parameters["kwargs"])

.. pyo3-api-summary:: ommx.experiment

.. toctree::
   :hidden:

   _items/ommx.experiment.Experiment
   _items/ommx.experiment.Run
   _items/ommx.experiment.SealedRun
   _items/ommx.experiment.Solve
