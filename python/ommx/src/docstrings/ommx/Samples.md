Multiple candidate assignments, each identified by a sample ID.

Each candidate is a {class}`~ommx.State`: a mapping from variable IDs to values.
Sample IDs identify candidates, independently of the variable IDs within each
candidate. Samples does not evaluate an objective or establish feasibility.
Pass it to {meth}`~ommx.Instance.evaluate_samples` to obtain a
{class}`~ommx.SampleSet`, just as evaluating one State produces a
{class}`~ommx.Solution`.

**Choose sample IDs or assign them automatically**

A dictionary from sample IDs to States or variable-value dictionaries preserves
the supplied IDs. Use these IDs to associate results with a sampler's records.
An iterable of States or variable-value dictionaries assigns IDs from 0 in
iteration order.

>>> from ommx import Samples, State
>>> explicit = Samples({10: {0: 1, 1: 0}, 20: State({0: 0, 1: 1})})
>>> assert explicit.sample_ids() == {10, 20}
>>> assert explicit.get_state(20).entries == {0: 0.0, 1: 1.0}
>>> automatic = Samples([{0: 1, 1: 0}, {0: 0, 1: 1}])
>>> assert automatic.sample_ids() == {0, 1}

Identical assignments may occur under different IDs. {meth}`~ommx.Samples.num_samples`
counts sample IDs, not distinct assignments; the IDs remain available after
evaluation.

>>> repeated = Samples([{0: 1}, {0: 1}])
>>> assert repeated.num_samples() == 2
>>> assert repeated.sample_ids() == {0, 1}
>>> assert repeated.get_state(0).entries == repeated.get_state(1).entries

**Accepted inputs**

The constructor also accepts an existing Samples object, one State, or a
nonempty variable-value dictionary. A single assignment receives sample ID 0.
An empty dictionary or empty iterable creates no samples; use ``[{}]`` or
``State({})`` to represent one empty assignment.

>>> single = Samples({5: 1.0})
>>> assert single.sample_ids() == {0}
>>> assert single.get_state(0).entries == {5: 1.0}
>>> assert Samples({}).num_samples() == 0
>>> assert Samples([{}]).num_samples() == 1
>>> assert Samples(State({})).num_samples() == 1

{meth}`~ommx.Instance.evaluate_samples` accepts these inputs directly, so an
explicit Samples construction is optional. {meth}`~ommx.Samples.append` adds
an assignment under one or more new IDs and rejects duplicate IDs without
changing the collection.
