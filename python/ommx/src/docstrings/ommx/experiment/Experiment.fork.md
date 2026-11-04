Fork this committed Experiment into a new unsealed child Experiment.

The parent is unchanged. Existing attachments, Runs, Solves, Samplings, and Run
parameters are carried into the child. The child reuses unchanged payload bytes
in the Local Registry, and its committed manifest records the parent's manifest
as OCI `subject`. An uncommitted parent raises an error.

If `image_name` is omitted, the child receives an anonymous local name. The
returned Experiment can be a context manager. Set `store_trace=True` to store
traces for Runs created from that child. The Experiment itself need not be a
context manager.

This complete example needs no solver or remote registry:

```pycon
>>> from ommx.experiment import Experiment
>>> with Experiment.with_temp_local_registry("example.com/fork:parent") as parent:
...     parent.log_json("dataset", {"name": "demo"})
...     with parent.run() as run:
...         run.log_parameter("trial", "baseline")
>>> with parent.fork("example.com/fork:child") as child:
...     with child.run() as run:
...         run.log_parameter("trial", "additional")
>>> len(parent.runs), len(child.runs)
(1, 2)
>>> child.get_json("dataset")
{'name': 'demo'}
>>> child.run_parameters_df()["trial"].to_list()
['baseline', 'additional']

```

A parent opened with {meth}`Experiment.load` or
{meth}`Experiment.import_archive` can be forked in the same way. A fork starts
new recording; it does not resume a solver's internal search state.

{meth}`Experiment.save` and {meth}`Experiment.push` share the entire child,
including inherited Runs and attachments. The child contains their payload
layers, so reading those records does not require separately opening the parent
Experiment.
