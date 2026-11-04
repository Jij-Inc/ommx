(python-sdk-v2-to-v3-migration-guide)=
# Migrate from Python SDK v2 to v3

This guide takes code written for [OMMX 2.9.4](https://pypi.org/project/ommx/2.9.4/) to the v3 API on `main`. Keep the v2 environment and set up a separate v3 environment using the [installation guide](../getting_started/install.md). Before changing the code, save one model, a candidate, and the results your application actually uses. You will compare them in {ref}`section 6 <migration-verify>`.

Start where your code uses OMMX. If you {ref}`construct models <migration-build>`, first decide who assigns IDs. If you {ref}`call a backend <migration-solve>`, check who prepares its inputs. If you only consume data, go straight to {ref}`interpreting results <migration-results>` or {ref}`loading saved data <migration-storage>`. To look up an error by its old method name, use the {ref}`API index <migration-index>`.

```{contents} Migration guide contents
:local:
:depth: 1
```

(migration-imports)=
## 1. Update imports and object access

### Use the v3 type stubs to find incompatible calls

Point your project's type checker at the environment containing OMMX v3 and run it on your application code. For a uv project with Pyright installed as a development dependency, run `uv run --no-sync pyright` from the project root. The SDK ships type stubs, so the checker can flag many changes before you run a solver.

| What Pyright flags | How to update the call |
|---|---|
| A list passed as `constraints` to `Instance.from_components` | Pass a mapping such as `constraints={7: x <= 1}` to preserve an external ID, or use {meth}`~ommx.Instance.add_constraint` for automatic numbering. |
| Unknown method `instance.write_mps` | Use {meth}`~ommx.Instance.save_mps`. |
| Missing argument in `solution.constraints_df()` | Select the constraint family, for example `kind=ConstraintKind.Regular`. |

Some changes keep the same type. For example, a value returned by {meth}`~ommx.Solution.extract_constraints` is still a float, but for a regular constraint it now measures violation. Use the {ref}`same-candidate comparison <migration-compare-model>` to check reports that depend on its meaning.

### Switch to the public imports

In v2.9.4, `Function`, `Constraint`, `DecisionVariable`, `Instance`, `ParametricInstance`, `Solution`, and `SampleSet` are already available from `ommx`. You can switch these imports while still running v2. In v3, import the other modeling types from the same public namespace, including `Linear`, `Parameter`, `Equality`, and `Sense`.

If your v2 code imports `ommx.v1.*_pb2` or accesses `.raw`, identify what it needs from the object. For example, read `instance.sense` directly. To build a model, use {meth}`~ommx.Instance.from_components` or the incremental API below. For serialized data, {ref}`select a reader by format <migration-storage>`. The v1 protobuf schema name still identifies a wire format. The Python SDK no longer provides those generated protobuf modules or the `from_raw`, `from_protobuf`, and `to_protobuf` wrappers.

(migration-build)=
## 2. Build and edit a model

(migration-automatic-ids)=
### Let the model allocate IDs, or preserve external IDs

If your v2 loop numbered variables and constraints solely to build a model, let the v3 {class}`~ommx.Instance` allocate them. Create the model with its objective sense first, then use the returned variables in expressions. The returned constraint handle carries its assigned ID.

```python
from ommx import Instance

model = Instance.maximize()
x = model.new_binary("x")
y = model.new_binary("y")
model.objective = x + y
limit = model.add_constraint(x + y <= 1, "limit")
assert model.constraints[limit.constraint_id].name == "limit"
```

`new_integer`, `new_continuous`, `new_semi_integer`, `new_semi_continuous`, and `new_finite_domain` also allocate variable IDs. On both `Instance` and `ParametricInstance`, `add_constraint`, `add_indicator_constraint`, `add_one_hot_constraint`, and `add_sos1_constraint` allocate constraint numbers.

If another file or application already refers to variable 0 or constraint 7, **preserve those numbers**. Keep the ID when constructing the variable. Replace v2's `constraints=[(x <= 1).set_id(7)]` with `constraints={7: x <= 1}` in `from_components`. A detached constraint has no ID. All constraint families share one number space per model, including removed constraints. Construction and loading reject collisions. {ref}`Section 4 <migration-compare-model>` uses an explicit ID in both versions.

(42-constraint-accessors-on-instance--parametricinstance--solution-return-dicts)=
(typed-constraint-ids-and-live-collections)=
### Find the row you intend to edit

Look up a regular constraint as `model.constraints[ConstraintID(7)]`. The collection contains all families, so a bare integer cannot identify the family for lookup. In a family-specific view, `model.constraints.of_kind(ConstraintKind.Regular)[7]` also works. Iteration yields typed IDs. Use `.items()` for ID/row pairs.

On a model, an active row is an attached handle. Calling `row.set_name("limit")` updates the model. Call `row.detach()` to keep an independent copy. Replace v2's `add_name` and `add_description` with `set_name` and `set_description`. Use `set_parameters` to replace the whole mapping and `add_parameters` to merge entries. Read fixed variable values from `model.fixed_decision_variables()`. The model owns that state even when a variable definition has been detached. See {class}`~ommx.ConstraintCollection` and {class}`~ommx.DecisionVariable`.

```{toctree}
:maxdepth: 1

v2_to_v3/constraint_metadata
```

### Carry over the mathematical structure

- **Constraint hints.** Reading v1 bytes preserves the regular constraints and ignores advisory `ConstraintHints`. If the original model specifies a OneHot or SOS1 requirement, you can construct the corresponding v3 special constraint directly. Removing the regular rows on the strength of a hint alone can change the feasible region.
- **Expressions.** Polynomial coefficients are available for expanded polynomials. A composed {class}`~ommx.Function` can return `None` from `degree()` or `num_terms()`. If you need an expanded square for coefficient export, multiply a polynomial by itself. Raising a `Function` to a power constructs a composed expression.
- **Parameters.** Create a parameter as `Parameter(3, name="capacity")` and pass an ID-to-value mapping to `with_parameters`. A {class}`~ommx.ParametricInstance` exposes parameter objects via `parameters` and a table via `parameters_df()`.

Check the type of any ID you exchange with other software. Variable and sample IDs are still integers. Constraint IDs stay typed in results and provenance and do not support `int()` conversion. For a custom transformation, use `ConstraintIDBase` and `ConstraintView` for heterogeneous rows and handle an unsupported family explicitly.

(migration-solve)=
## 3. Pass a model to a solver or sampler

For the adapter's recommended preparation, pass your model to `solve()` or `sample()`. These methods prepare a copy. For a custom transformation policy, call `instance.prepare(adapter.INPUT_CLASS, policy)` on an instance you intend to change, then call `solve_without_preparation()` or `sample_without_preparation()` on that prepared input. The [adapter catalogue](../api/adapters) links to each backend's inputs and executable workflow.

**Initial candidates and penalties.** SCIP still accepts initial values by variable name and subscripts, as in 2.9.4. OpenJij initial states use the solver variable IDs of the exact prepared input. Supply them to its preparation-free API. When constraints require fixed penalties, choose the weights explicitly. Move the v2 penalty arguments to `policy.fixed_penalty` and integer slack settings to `policy.integer_slack`. The {class}`OpenJij adapter reference <ommx_openjij_adapter.OMMXOpenJijSAAdapter>` shows the full sequence.

**Exporting coefficients yourself.** {meth}`~ommx.Instance.to_qubo` and {meth}`~ommx.Instance.to_hubo` mutate the instance and expose the energy sent to the backend. In v3, evaluating a candidate with this instance returns the objective it exposed before preparation. Keep the backend energy separately if your application compares penalties or solver scores.

```{toctree}
:maxdepth: 1

v2_to_v3/openjij_penalties
```

### If you implement an adapter

Declare the accepted input as `INPUT_CLASS` and implement `solve_without_preparation()` or `sample_without_preparation()`. Expose backend options through explicit typed arguments. An option keyed to prepared solver coordinates belongs on the preparation-free call unless the adapter can map it across transformations. Reject constraint families your backend cannot represent. Skipping a row can admit solutions that violate the model. See {class}`~ommx.adapter.SolverAdapter` and {class}`~ommx.adapter.SamplerAdapter`.

(migration-results)=
## 4. Evaluate candidates and read results

(migration-compare-model)=
(constraint-violation-migration)=
### Does your report need the signed value or the violation?

For the candidate $x=0$, the constraint $x-1\leq0$ has signed value $-1$ and violation $0$. Both SDKs agree the candidate satisfies it. The meaning of the number returned by name has changed.

**v2.9.4**

```python
from ommx import DecisionVariable, Instance

x = DecisionVariable.binary(0)
limit = (x <= 1).set_id(7).add_name("limit")
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints=[limit], sense=Instance.MAXIMIZE,
)
result = model.evaluate({0: 0})
assert result.extract_constraints("limit")[()] == -1.0
```

**v3 on `main`**

```python
from ommx import ConstraintID, DecisionVariable, Instance, Sense

x = DecisionVariable.binary(0)
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints={7: x <= 1}, sense=Sense.Maximize,
)
model.constraints[ConstraintID(7)].set_name("limit")
result = model.evaluate({0: 0})
assert result.extract_constraints("limit")[()] == 0.0
assert result.get_constraint_value(ConstraintID(7)) == -1.0
```

{meth}`~ommx.Solution.extract_constraints` now searches all families and returns nonnegative violations keyed by subscript tuple. Duplicate subscripts for a name are ambiguous, even across families. If you relied on the sign or slack of a regular row, use `get_constraint_value` as above. For a violation by typed ID use `constraint_violation`. When reading a single evaluated row, use `row.is_feasible(atol=result.feasibility_atol)` to apply the result's tolerance. Overall feasibility also checks variable domains. A zero total constraint violation alone is insufficient.

For a regular equality, the violation is the absolute residual. For an inequality it is the excess above the limit. Special constraints have family-specific metrics. `total_violation()` adds these values, including retained removed rows. Adding rows during transformation can therefore change the total. See {meth}`~ommx.Solution.total_violation` for definitions and tolerances. Update error handling for a missing required variable in `evaluate()` to catch `ValueError`. Partial assignments remain valid inputs to `partial_evaluate()`.

### Update tables and sample aggregation

Call the `*_df` methods. In particular, replace `solution.constraints_df` with `solution.constraints_df(kind=ConstraintKind.Regular)`. Constraint indexes contain family-specific ID objects. Update joins that expected integers. For a model, `removed=True` includes both active and retained removed rows. Results already include retained removed constraints and return snapshots on lookup. The constraint metadata tables for context, parameters, provenance, and removal reasons also require `kind=`.

For multiple samples, use `samples.summary` for objectives and overall feasibility, and `samples.constraints_df(kind=...)` for per-constraint results. `samples.sample_ids()` returns a set. Use `samples.sample_ids_list` when the consumer needs a list. See {class}`~ommx.Solution` and {class}`~ommx.SampleSet` for complete examples.

```{toctree}
:maxdepth: 1

v2_to_v3/constraint_tables
```

(migration-storage)=
## 5. Bring saved models and Artifacts forward

### Read a serialized model or result

The SDK version and the protobuf version are separate. A payload written by `Instance.to_bytes()` in **SDK 2.9.4** uses the **v1 wire format**. Read it with `Instance.from_v1_bytes(data)` in v3. The same naming applies to `ParametricInstance`, `Solution`, and `SampleSet`. For new files use `to_v2_bytes()` and the matching `from_v2_bytes()` on the owning object. To persist a variable, expression, or constraint with its context, serialize the enclosing model or result.

Version 2.9.4 already preserves annotations in root payloads. In v3, set typed metadata through properties such as `title` and use `add_user_annotation` for extension keys. Update any direct edits to the `annotations` dictionary.

If a 2.x program must still consume your output, check that it supports the representation as well as the wire format. For example, 2.9.4's schema includes finite-domain variables but its SDK rejects them when loading a model. An explicit output objective cannot be written with `to_v1_bytes()`. Use the v2 wire format to preserve native special constraints and a result's feasibility tolerance.

If you export MPS to another tool, replace `write_mps` with {meth}`~ommx.Instance.save_mps`. Use the SDK byte format to restore model structure and metadata that MPS cannot represent.

(13-artifact-api-archive-becomes-an-exchange-format)=
### Open or create a `.ommx` archive

| What your v2 code did | What to do in v3 |
|---|---|
| Opened an archive with `Artifact.load_archive(path)` | To inspect its manifest and layer descriptors, use `Artifact.inspect_archive(path)`. To read the layer contents, use `Artifact.import_archive(path)`, which writes to the local registry. |
| Built a file with `ArtifactBuilder.new_archive(path, name)` | Create an `ArtifactDraft.new(name)`, add the layers, commit, and call `artifact.save(path)` to export the file. |
| Used `new_archive_unnamed(path)` | Use `ArtifactDraft.new_anonymous()`. A committed anonymous Artifact has a generated image name. Update code that checked `image_name is None`. |
| Pushed a file with `ommx push` | Import it with `ommx load`, then push by image name. |

See {class}`~ommx.artifact.Artifact` for the archive API. For cleanup, `ommx prune-anonymous` previews refs and `ommx gc` previews unreachable blobs. Each requires `--delete` to perform deletion. See [Manage the Local Registry](../how_to/manage_local_registry.md).

```{toctree}
:maxdepth: 1

v2_to_v3/artifact_archives
```

(migration-verify)=
## 6. Verify your application's results

1. Evaluate the same candidate against the same original model in both environments. Compare objective sense, objective value, feasibility, and the tolerance used. For a transformed model, compare each result against the original requirements.
2. Compare the numbers your application consumes. For example, a report that used the signed value $-1$ in section 4 should still read $-1$ after migration. A report intended to measure violation should read $0$ for that candidate. Compare sample IDs and join keys if your application aggregates results in tables.
3. If your workflow crosses a file or process boundary, read a real 2.9.4 payload or archive in v3. Check an external ID, a metadata field, and an evaluation result. If a 2.x consumer remains in service, exercise that reader on the outbound format as well.

(migration-index)=
## Appendix. Find a replacement by its v2 name

The site-wide {ref}`general index <genindex>` also lists the detailed examples under their v2 API names.

| Old API or use | v3 replacement | Section |
|---|---|---|
| `.raw` / `from_raw` / `from_protobuf` / `to_protobuf` | Public properties and constructors | {ref}`1 <migration-imports>` |
| `add_name` / `add_description` | `set_name` / `set_description` | {doc}`v2_to_v3/constraint_metadata` |
| `ArtifactArchive` / `ArtifactDir` / `ArtifactBuilder` | `Artifact` / `ArtifactDraft` | {doc}`v2_to_v3/artifact_archives` |
| `Artifact.load_archive` | `Artifact.import_archive` / `Artifact.inspect_archive` | {doc}`v2_to_v3/artifact_archives` |
| `Constraint.id` / `set_id` / `next_constraint_id` | `add_constraint` / `from_components` | {ref}`2 <migration-build>` |
| `constraint_hints` / `OneHot` / `Sos1` | `OneHotConstraint` / `Sos1Constraint` | {ref}`2 <migration-build>` |
| `constraints_df` / `removed_constraints_df` | `constraints_df(kind=...)` / `constraints_df(kind=..., removed=True)` | {doc}`v2_to_v3/constraint_tables` |
| `decision_variable_analysis()` | `decision_variable_roles()` / `fixed_decision_variables()` | {doc}`v2_to_v3/constraint_metadata` |
| `DecisionVariable.substituted_value` | `model.fixed_decision_variables()` | {ref}`2 <migration-build>` |
| `evaluated.feasible` / `sampled.feasible` | `evaluated.is_feasible(atol=...)` / `sampled.feasible(atol=...)` | {ref}`4 <migration-results>` |
| `extract_constraints(name)` | extract_constraints for violations, get_constraint_value for signed regular values | {ref}`4 <migration-results>` |
| `get_constraint_by_id` / `get_removed_constraint_by_id` | `constraints[ConstraintID(n)]` / `removed_constraints[ConstraintID(n)]` | {ref}`2 <migration-build>` |
| `Linear.from_object` / `Linear.equals_to` | arithmetic or Linear.single_term / almost_equal | {ref}`2 <migration-build>` |
| `Linear.terms` / `Quadratic.terms` / `Polynomial.terms` | `terms()` | {ref}`2 <migration-build>` |
| `ommx.v1` / `ommx.v1.*_pb2` | `ommx` | {ref}`1 <migration-imports>` |
| `Parameter.new` / `Parameters(entries=...)` | `Parameter(id, ...)` / `with_parameters({id: value})` | {ref}`2 <migration-build>` |
| `ParametricInstance.parameters (DataFrame)` | `parameters_df()` | {ref}`2 <migration-build>` |
| `response_to_samples` / `sample_qubo_sa` | `decode_to_samples` / `OMMXOpenJijSAAdapter.sample` | {doc}`v2_to_v3/openjij_penalties` |
| `SampleSet.sample_ids` / `summary_with_constraints` | `sample_ids()` / `summary + constraints_df(kind=...)` | {ref}`4 <migration-results>` |
| `Solution.total_violation_l1` / `total_violation_l2` | total_violation (sum of per-constraint violations); no L2 aggregate | {ref}`4 <migration-results>` |
| `to_bytes` / `from_bytes` | `to_v2_bytes` / `from_v2_bytes` / `from_v1_bytes` | {ref}`5 <migration-storage>` |
| `uniform_penalty_weight` / `penalty_weights` / `inequality_integer_slack_max_range` | `policy.fixed_penalty` / `policy.integer_slack` | {doc}`v2_to_v3/openjij_penalties` |
| `used_decision_variable_ids() (Instance, Function)` | `required_ids()` | {ref}`2 <migration-build>` |
| `write_mps(path)` | `save_mps(path)` | {ref}`5 <migration-storage>` |

For changes between individual v3 prereleases, see the [Python SDK 3.0 release notes](../release_note/ommx-3.0.md).
