(python-sdk-v2-to-v3-migration-guide)=
# Migrate from Python SDK v2 to v3

This guide takes code written for [OMMX 2.9.4](https://pypi.org/project/ommx/2.9.4/) to the v3 API on `main`. Keep the v2 environment and set up a separate v3 environment using the [installation guide](../getting_started/install.md). Before changing the code, save one model, a candidate, and the results your application actually uses. You will compare them in {ref}`section 6 <migration-verify>`.

**Start by running a type checker on one of your existing scripts.** Even if you have never used one, you can use its diagnostics to fix many incompatible calls immediately. {ref}`Run Pyright for the first time <migration-first-type-check>`, then use the {ref}`old API index <migration-index>` to look up a name in its diagnostics.

```{contents} Migration guide contents
:local:
:depth: 1
```

(migration-imports)=
## 1. Update imports and object access

(migration-first-type-check)=
### Run a type checker on your first script

From the project directory where you installed v3, run the command below. Replace `solve.py` with the path to a Python script that uses OMMX. uv provides Pyright for this command. You can start with an ordinary script, without adding type annotations or a configuration file.

```sh
uv run --no-sync --with pyright pyright solve.py
```

Pyright reads the type information shipped with OMMX and reports incompatible calls with their file and line numbers without executing the script. Fix any old `ommx.v1` imports first so Pyright can identify the SDK objects. For example, if it flags `instance.write_mps(path)` as an unknown method, change it to `instance.save_mps(path)` and run the same command again. Work through the remaining diagnostics, then check the next script.

| What Pyright flags | How to update the call |
|---|---|
| A list passed as `constraints` to `Instance.from_components` | Add each constraint to the model with {meth}`~ommx.Instance.add_constraint`. |
| Unknown method `instance.write_mps` | Use {meth}`~ommx.Instance.save_mps`. |
| Missing argument in `solution.constraints_df()` | Select the constraint family, for example `kind=ConstraintKind.Regular`. |

Some changes keep the same type. For example, a value returned by {meth}`~ommx.Solution.extract_constraints` is still a float, but for a regular constraint it now measures violation. Use the {ref}`same-candidate comparison <migration-compare-model>` to check reports that depend on its meaning.

### Switch to the public imports

In v2.9.4, `Function`, `Constraint`, `DecisionVariable`, `Instance`, `ParametricInstance`, `Solution`, and `SampleSet` are already available from `ommx`. You can switch these imports while still running v2. In v3, import the other modeling types from the same public namespace, including `Linear`, `Parameter`, `Equality`, and `Sense`.

If your v2 code imports `ommx.v1.*_pb2` or accesses `.raw`, identify what it needs from the object. For example, read `instance.sense` directly. Start a model with {meth}`~ommx.Instance.minimize` or {meth}`~ommx.Instance.maximize`, then add variables and constraints. For serialized data, {ref}`select a reader by format <migration-storage>`. The v1 protobuf schema name still identifies a wire format. The Python SDK no longer provides those generated protobuf modules or the `from_raw`, `from_protobuf`, and `to_protobuf` wrappers.

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
assert model.active_constraints[limit.constraint_id].name == "limit"
```

`new_integer`, `new_continuous`, `new_semi_integer`, `new_semi_continuous`, and `new_finite_domain` also allocate variable IDs. On both `Instance` and `ParametricInstance`, `add_constraint`, `add_indicator_constraint`, `add_one_hot_constraint`, and `add_sos1_constraint` allocate constraint numbers.

If an external consumer requires the model itself to keep variable 0 or constraint 7, **preserve those numbers**. Keep the ID when constructing the variable. Replace v2's `constraints=[(x <= 1).set_id(7)]` with `active_constraints={7: x <= 1}` in {meth}`~ommx.Instance.from_components`. A detached constraint has no ID. All constraint families share one number space per model, including inactive constraints. Construction and loading reject collisions.

(42-constraint-accessors-on-instance--parametricinstance--solution-return-dicts)=
(typed-constraint-ids-and-live-collections)=
### Find the row you intend to edit

Look up a regular constraint as `model.active_constraints[ConstraintID(7)]`. The collection contains all families, so a bare integer cannot identify the family for lookup. In a family-specific view, `model.active_constraints.of_kind(ConstraintKind.Regular)[7]` also works. Iteration yields typed IDs. Use `.items()` for ID/row pairs.

On a model, an active row is an attached handle. Calling `row.set_name("limit")` updates the model. Call `row.detach()` to keep an independent copy. Replace v2's `add_name` and `add_description` with `set_name` and `set_description`. Use `set_parameters` to replace the whole mapping and `add_parameters` to merge entries. Read fixed variable values from `model.fixed_decision_variables()`. The model owns that state even when a variable definition has been detached. See {class}`~ommx.ActiveConstraintCollection` and {class}`~ommx.DecisionVariable`.

(migration-constraint-metadata)=
### Check where a metadata edit lands

```{index} Constraint.add_name (v2), Constraint.add_description (v2), Instance.get_constraint_by_id (v2), Instance.decision_variable_analysis (v2)
```

In 2.9.4, editing the constraint returned by `get_constraint_by_id` only modified that Python snapshot. When translating this code, decide whether you intended to edit the model or just a copy for display. Both operations are explicit in v3.

```python
from ommx import Instance

model = Instance.minimize()
x = model.new_binary("x")
live = model.add_constraint(x <= 1, "original")
local = live.detach()
local.set_name("display")
assert model.active_constraints[live.constraint_id].name == "original"
live.set_name("capacity")
assert model.active_constraints[live.constraint_id].name == "capacity"
```

A dictionary made with `dict(model.active_constraints)` still holds attached rows. Detach each active row you want to copy. On a detached constraint, a setter changes the receiver and returns a clone. To apply several edits to the same detached object, call its setters separately.

### Replace a variable analysis query

For one variable, use {meth}`~ommx.Instance.decision_variable_role`. For a report over many variables, use `decision_variables_df()` and its `state_role` and `substituted_value` columns. If your adapter only needs variables sent to the solver, use `active_decision_variables`. These queries answer different questions when fixed or dependent variables remain available for evaluating candidates.

Replace `used_decision_variables` with `active_decision_variables` and `DecisionVariableRole.Used` with `DecisionVariableRole.Active`; the `state_role` value is now `"active"`. Adapters use the Instance's active variables and active constraints as its Active Formulation. The Instance derives the active variable set from its current objective and active constraints. Row-level `used_decision_variable_ids` and DataFrame `used_ids` still describe references within that row, including inactive rows.

The Preparation selector is also named `IntegerEncodingPreparation.log_encode_all_active_integers()`, replacing `log_encode_all_used_integers()`.

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

(migration-openjij-penalties)=
### Move OpenJij penalty options into the preparation policy

```{index} sample_qubo_sa (v2), uniform_penalty_weight (v2), penalty_weights (v2), inequality_integer_slack_max_range (v2)
```

If your v2 call passed `uniform_penalty_weight` or relied on the implicit weight of 1, choose a weight for the original objective scale. Here selecting both options gains two points over the best allowed choice. A weight of four offsets that gain. Keep the source model so that another weight can be tried on a fresh copy. Install the OpenJij adapter to run this example.

```python
from copy import copy

from ommx import FixedPenaltyPreparation, Instance, Sense
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

source = Instance.maximize()
x, y = source.new_binary("x"), source.new_binary("y")
source.objective = 3 * x + 2 * y
source.add_constraint(x + y == 1)
prepared = copy(source)
policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = (
    FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=4.0)
)
prepared.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)
samples = OMMXOpenJijSAAdapter.sample_without_preparation(prepared, num_reads=2, seed=42)
assert samples.sense == Sense.Maximize
```

The sampling result reports the original objective. If your application also reads dual values, note that HiGHS and Python-MIP omit them when the prepared input has an explicit output objective.

### Update downstream consumers

The v2 helper `sample_qubo_sa` returned raw samples. The v3 adapter returns an evaluated {class}`~ommx.SampleSet`. For code that already has a native OpenJij response and only needs to decode it, replace `response_to_samples` with `decode_to_samples`. If you change a penalty, start again from the original model: preparing an already applicable input does not replace the penalty stored in its objective.

### If you implement an adapter

Declare the accepted input as `INPUT_CLASS` and implement `solve_without_preparation()` or `sample_without_preparation()`. Expose backend options through explicit typed arguments. An option keyed to prepared solver coordinates belongs on the preparation-free call unless the adapter can map it across transformations. Reject constraint families your backend cannot represent. Skipping a row can admit solutions that violate the model. See {class}`~ommx.adapter.SolverAdapter` and {class}`~ommx.adapter.SamplerAdapter`.

(migration-results)=
## 4. Evaluate candidates and read results

### Select constraints by role

Replace `relax_constraint` / `restore_constraint` with `deactivate_constraint` / `activate_constraint`, and `RemovedConstraint` with `InactiveConstraint`. The same naming applies to indicator, OneHot, and SOS1 families. Deactivation retains the row and its reason; `skip_feasibility_check=False` is the default. A row with `skip_feasibility_check=True` is still numerically evaluated, and evaluation errors still propagate.

`Instance.constraints` and `ParametricInstance.constraints` now include every role. A typed ID identifies the family, not the role: lookup returns the common `ConstraintView`, whose concrete runtime type depends on the row's current active/inactive membership. Use `active_constraints` for typed attached handles and `inactive_constraints` for inactive snapshots. `Solution.constraints` and `SampleSet.constraints` continue to contain all evaluated rows.

On `Instance` and `ParametricInstance`, rename `constraint_names` to `active_constraint_names` and `extract_constraints(name)` to `extract_active_constraints(name)`. Both queries still select only active constraints, and their return types are unchanged. `Solution` and `SampleSet` keep `constraint_names` and `extract_constraints`.

For `from_components`, rename `constraints=` to `active_constraints=`, and similarly use `active_indicator_constraints=`, `active_one_hot_constraints=`, and `active_sos1_constraints=`. Initial inactive rows use the corresponding `inactive_*` arguments and `Inactive*Constraint` wrappers.

Each row's `role` property is a `ConstraintRole`: `EVALUATION_ONLY` uses the row for numerical evaluation, `FEASIBILITY_CHECK` additionally uses it for normal feasibility, and `ACTIVE` additionally includes it in the Active Formulation. These roles describe participation, not whether evaluation has already occurred. **Inactive** is the umbrella term for the first two roles; `inactive_constraints` contains both. An inactive row has role `FEASIBILITY_CHECK` when `skip_feasibility_check=False`, and `EVALUATION_ONLY` when it is `True`.

Indicator result properties and DataFrame columns use `indicator_on` to report whether the indicator variable matches the configured `trigger_value` within the evaluation tolerance. The trigger value is 0 or 1, with a default of 1. This describes the evaluated candidate, independently of the constraint's role.

OneHot and SOS1 result properties rename `active_variable` to `selected_variable`, including per-sample mappings. DataFrame columns become `selected_variable` and `selected_variable.<sample_id>`. This diagnostic describes selection at the evaluated candidate, independently of variable or constraint membership in the Active Formulation. Evaluation behavior is unchanged.

The `min_role` selector for `total_violation()` and `constraints_df` is an ordered minimum, so it can include more than one role:

| `min_role` | Included constraints |
| --- | --- |
| `ConstraintRole.ACTIVE` | Active only |
| `ConstraintRole.FEASIBILITY_CHECK` | Active and inactive with `skip_feasibility_check=False` |
| `ConstraintRole.EVALUATION_ONLY` | Every retained constraint |

The order is `EVALUATION_ONLY < FEASIBILITY_CHECK < ACTIVE`; a query includes rows whose role is at least the minimum. It cannot include `EVALUATION_ONLY` rows while excluding `FEASIBILITY_CHECK` rows.

Read `solution.feasible` and `samples.feasible` as properties. They check constraints with the `FEASIBILITY_CHECK` or `ACTIVE` role; `feasible_relaxed` checks active constraints only. SampleSet's `best_feasible` and `best_feasible_id` remain properties, while `feasible_ids()` remains a method without arguments. Feasibility and sample selection do not accept `min_role`. The default `total_violation()` uses `FEASIBILITY_CHECK`, matching the constraints checked by `feasible`; selecting `ACTIVE` matches `feasible_relaxed`. `constraints_df` defaults to `EVALUATION_ONLY`. Feasibility applies each row's tolerance and checks variable domains; do not infer it from an exact-zero test of the total.

The v1 wire schema is unchanged. Export to v1 fails if any retained row has `skip_feasibility_check=True`; use v2 to preserve this state. Existing restrictions on v1 exports still apply.

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
from ommx import Instance

model = Instance.maximize()
x = model.new_binary("x")
model.objective = x
limit = model.add_constraint(x <= 1, "limit")
result = model.evaluate({x.id: 0})
assert result.extract_constraints("limit")[()] == 0.0
assert result.get_constraint_value(limit.constraint_id) == -1.0
```

{meth}`~ommx.Solution.extract_constraints` now searches all families and returns nonnegative violations keyed by subscript tuple. Duplicate subscripts for a name are ambiguous, even across families. If you relied on the sign or slack of a regular row, use `get_constraint_value` as above. For a violation by typed ID use `constraint_violation`. When reading a single evaluated row, use `row.is_feasible(atol=result.feasibility_atol)` to apply the result's tolerance. Overall feasibility also checks variable domains. A zero total constraint violation alone is insufficient.

For a regular equality, the violation is the absolute residual. For an inequality it is the excess above the limit. Special constraints have family-specific metrics. `total_violation()` adds these values for the constraints checked by `feasible`; pass `min_role=ConstraintRole.ACTIVE` to use the constraints checked by `feasible_relaxed`. Adding rows during transformation can therefore change the total. See {meth}`~ommx.Solution.total_violation` for definitions and tolerances. Update error handling for a missing required variable in `evaluate()` to catch `ValueError`. Partial assignments remain valid inputs to `partial_evaluate()`.

### Update tables and sample aggregation

Call the `*_df` methods. In particular, replace `solution.constraints_df` with `solution.constraints_df(kind=ConstraintKind.Regular)`. Constraint indexes contain family-specific ID objects. Update joins that expected integers. On models and results, `constraints_df(kind=...)` includes rows of every role by default. Pass `min_role=ConstraintRole.ACTIVE` for active rows only. Result collections return snapshots on lookup. The constraint metadata tables for context, parameters, provenance, and inactivation reasons also require `kind=`.

For multiple samples, use `samples.summary` for objectives and overall feasibility, and `samples.constraints_df(kind=...)` for per-constraint results. `samples.sample_ids()` returns a set. Use `samples.sample_ids_list` when the consumer needs a list. See {class}`~ommx.Solution` and {class}`~ommx.SampleSet` for complete examples.

(migration-constraint-tables)=
### Migrate a join on constraint ID

```{index} Instance.constraints_df (v2), Instance.removed_constraints_df (v2), SampleSet.summary_with_constraints (v2)
```

In v2 a report might join the `id` index of `instance.constraints_df` to a table keyed by integers. In v3 the regular-constraint index is named `regular_constraint_id` and contains `ConstraintID` objects. When building the model, record which assigned ID corresponds to each source constraint number (here, 7). Use that mapping to join the application's table.

```python
import pandas as pd
from ommx import ConstraintKind, Instance

model = Instance.maximize()
x = model.new_binary("x")
model.objective = x
limit = model.add_constraint(x <= 1, "limit")
id_map = {7: limit.constraint_id}

owners = pd.Series({7: "planning"}, name="owner")
rows = model.constraints_df(kind=ConstraintKind.Regular, include=[])
report = rows.join(owners.rename(index=id_map))
assert report.loc[limit.constraint_id, "owner"] == "planning"
```

Use `include=["label", "parameters"]` for a wide table with metadata. For a join with one row per key/value, use `constraint_parameters_df(kind=...)`. Transformation history is in `constraint_provenance_df(kind=...)`, with one row per provenance step. The `role` column is always present. Filter out `ConstraintRole.ACTIVE` if your old report contained only removed rows. Request `include=["inactive_reason"]` to add the reason and its parameters, or use `constraint_inactive_reasons_df(kind=...)` for a long table.

### Compare feasibility near the tolerance

A threshold is applied to each constraint's violation. For SOS1, leaving at most one nonzero value in `(1, 0.00006, 0.00006)` takes total absolute change `0.00012`. At a tolerance of `0.0001` the row fails, even though each of the two small values is below that threshold. When reconciling reports, use the stored evaluation tolerance and compare the same constraint family. A new query tolerance does not rerun discrete-value canonicalization. See {meth}`~ommx.Solution.total_violation`.

(migration-storage)=
## 5. Bring saved models and Artifacts forward

### Read a serialized model or result

The SDK version and the protobuf version are separate. A payload written by `Instance.to_bytes()` in **SDK 2.9.4** uses the **v1 wire format**. Read it with `Instance.from_v1_bytes(data)` in v3. The same naming applies to `ParametricInstance`, `Solution`, and `SampleSet`. For new files use `to_v2_bytes()` and the matching `from_v2_bytes()` on the owning object. To persist a variable, expression, or constraint with its context, serialize the enclosing model or result.

`from_v1_bytes()` is the compatibility import for reusing data saved by older SDKs. Its policy is to recover as much existing data as possible through legacy defaults, normalization, and reconstruction of derived values, while preserving mathematical meaning. Missing source data must not be invented, and ambiguous mathematical content remains an error.

Current recovery is limited to implemented rules. For example, `Solution` can restore a missing state value from a saved variable value, and `SampleSet` reconstructs omitted root feasibility maps. However, both still reject saved feasibility judgments that disagree with recomputation at the SDK default tolerance. V1 does not record the producer's tolerance. Further healing is tracked in [issue #1381](https://github.com/Jij-Inc/ommx/issues/1381); the documentation of this policy does not change parsing behavior.

Version 2.9.4 already preserves annotations in root payloads. In v3, set typed metadata through properties such as `title` and use `add_user_annotation` for extension keys. Update any direct edits to the `annotations` dictionary.

Use `to_v1_bytes()` when you need to send data to an older OMMX SDK, such as SDK 2.x. It is a compatibility serialization format: only data representable in v1 can be exported. V3 features such as native Indicator, OneHot, and SOS1 constraints cannot be represented, so serialization raises `RuntimeError`. This applies to `Instance`, `ParametricInstance`, `Solution`, and `SampleSet`, including constraints retained as inactive after lowering. An explicit output objective also prevents model export. Use `to_v2_bytes()` to preserve these features.

Even when v1 serialization succeeds, check that the receiving SDK supports the representation. For example, 2.9.4's schema includes finite-domain variables but its SDK rejects them when loading a model. For results that can be exported, v1 recomputes feasibility at the SDK default tolerance; v2 preserves the evaluation tolerance. Reading existing v1 payloads is unchanged.

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

(migration-artifact-archives)=
### Replace an archive builder and reopen its output

```{index} ArtifactBuilder.new_archive (v2), ArtifactBuilder.new_archive_unnamed (v2), Artifact.load_archive (v2)
```

A v2 archive builder wrote a file when `build()` was called. In v3, `commit()` writes to the local registry and `save()` exports a file. This example creates an anonymous registry entry and uses a fresh directory for the archive.

```python
from pathlib import Path
from tempfile import TemporaryDirectory

from ommx import Instance
from ommx.artifact import Artifact, ArtifactDraft

model = Instance.minimize()
model.title = "Archive migration"
draft = ArtifactDraft.new_anonymous()
draft.add_instance(model)
artifact = draft.commit()
with TemporaryDirectory() as directory:
    path = Path(directory) / "model.ommx"
    artifact.save(path)
    manifest = Artifact.inspect_archive(path)
    assert manifest.image_name == artifact.image_name
    restored = Artifact.import_archive(path).get_instance()
    assert restored.title == model.title
```

For an existing unnamed v2 archive without an image-name annotation, `import_archive` assigns an anonymous local name. Read the returned Artifact's `image_name` if a later step needs to load it by name. Choose a fresh output path when exporting: `save` refuses to overwrite an existing file.

### Read files from another SDK

Use {meth}`~ommx.artifact.Artifact.inspect_archive` to inventory an archive without writing to the local registry. The manifest describes layers, but reading a model from a layer requires {meth}`~ommx.artifact.Artifact.import_archive`. For old archives with annotations on layer descriptors, protobuf metadata takes precedence if both sources specify the same OMMX key. Check titles and application annotations when migrating stored experiments.

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
| `.raw` / `from_raw` / `from_protobuf` / `to_protobuf` | For example, {attr}`instance.sense <ommx.Instance.sense>` / {meth}`Instance.minimize <ommx.Instance.minimize>` | {ref}`1 <migration-imports>` |
| `add_name` / `add_description` | {meth}`set_name <ommx.Constraint.set_name>` / {meth}`set_description <ommx.Constraint.set_description>` | {ref}`migration-constraint-metadata` |
| `ArtifactArchive` / `ArtifactDir` / `ArtifactBuilder` | {class}`Artifact <ommx.artifact.Artifact>` / {class}`ArtifactDraft <ommx.artifact.ArtifactDraft>` | {ref}`migration-artifact-archives` |
| `Artifact.load_archive` | {meth}`Artifact.import_archive <ommx.artifact.Artifact.import_archive>` / {meth}`Artifact.inspect_archive <ommx.artifact.Artifact.inspect_archive>` | {ref}`migration-artifact-archives` |
| `Constraint.id` / `set_id` / `next_constraint_id` | {meth}`add_constraint <ommx.Instance.add_constraint>` or, to preserve external constraint numbers, {meth}`from_components <ommx.Instance.from_components>` | {ref}`2 <migration-build>` |
| `constraint_hints` / `OneHot` / `Sos1` | {class}`OneHotConstraint <ommx.OneHotConstraint>` / {class}`Sos1Constraint <ommx.Sos1Constraint>` | {ref}`2 <migration-build>` |
| `constraints_df` / `removed_constraints_df` | {meth}`constraints_df(kind=..., min_role=...) <ommx.Instance.constraints_df>`; filter `role` for inactive-only reports | {ref}`migration-constraint-tables` |
| `decision_variable_analysis()` | {meth}`decision_variable_roles() <ommx.Instance.decision_variable_roles>` / {meth}`fixed_decision_variables() <ommx.Instance.fixed_decision_variables>` | {ref}`migration-constraint-metadata` |
| `DecisionVariable.substituted_value` | {meth}`model.fixed_decision_variables() <ommx.Instance.fixed_decision_variables>` | {ref}`2 <migration-build>` |
| `evaluated.feasible` / `sampled.feasible` | {meth}`evaluated.is_feasible(atol=...) <ommx.EvaluatedConstraint.is_feasible>` / {meth}`sampled.feasible(atol=...) <ommx.SampledConstraint.feasible>` | {ref}`4 <migration-results>` |
| `extract_constraints(name)` | {meth}`extract_constraints <ommx.Solution.extract_constraints>` for violations, {meth}`get_constraint_value <ommx.Solution.get_constraint_value>` for signed regular values | {ref}`4 <migration-results>` |
| `get_constraint_by_id` / `get_removed_constraint_by_id` | {attr}`active_constraints[ConstraintID(n)] <ommx.Instance.active_constraints>` / {attr}`inactive_constraints[ConstraintID(n)] <ommx.Instance.inactive_constraints>` | {ref}`2 <migration-build>` |
| `Linear.from_object` / `Linear.equals_to` | Arithmetic or {meth}`Linear.single_term <ommx.Linear.single_term>` / {meth}`almost_equal <ommx.Linear.almost_equal>` | {ref}`2 <migration-build>` |
| `Linear.terms` / `Quadratic.terms` / `Polynomial.terms` | {meth}`Linear.terms() <ommx.Linear.terms>` / {meth}`Quadratic.terms() <ommx.Quadratic.terms>` / {meth}`Polynomial.terms() <ommx.Polynomial.terms>` | {ref}`2 <migration-build>` |
| `ommx.v1` / `ommx.v1.*_pb2` | {doc}`ommx </api/index>` | {ref}`1 <migration-imports>` |
| `Parameter.new` / `Parameters(entries=...)` | {class}`Parameter(id, ...) <ommx.Parameter>` / {meth}`with_parameters({id: value}) <ommx.ParametricInstance.with_parameters>` | {ref}`2 <migration-build>` |
| `ParametricInstance.parameters (DataFrame)` | {meth}`parameters_df() <ommx.ParametricInstance.parameters_df>` | {ref}`2 <migration-build>` |
| `response_to_samples` / `sample_qubo_sa` | {func}`decode_to_samples <ommx_openjij_adapter.decode_to_samples>` / {meth}`OMMXOpenJijSAAdapter.sample <ommx_openjij_adapter.OMMXOpenJijSAAdapter.sample>` | {ref}`migration-openjij-penalties` |
| `SampleSet.sample_ids` / `summary_with_constraints` | {meth}`sample_ids() <ommx.SampleSet.sample_ids>` / {attr}`summary <ommx.SampleSet.summary>` + {meth}`constraints_df(kind=...) <ommx.SampleSet.constraints_df>` | {ref}`4 <migration-results>` |
| `Solution.total_violation_l1` / `total_violation_l2` | {meth}`total_violation <ommx.Solution.total_violation>` (sum of per-constraint violations). The L2 aggregate was removed. | {ref}`4 <migration-results>` |
| `to_bytes` / `from_bytes` | {meth}`to_v2_bytes <ommx.Instance.to_v2_bytes>` / {meth}`from_v2_bytes <ommx.Instance.from_v2_bytes>` / {meth}`from_v1_bytes <ommx.Instance.from_v1_bytes>` | {ref}`5 <migration-storage>` |
| `uniform_penalty_weight` / `penalty_weights` / `inequality_integer_slack_max_range` | {attr}`policy.fixed_penalty <ommx.PreparationPolicy.fixed_penalty>` / {attr}`policy.integer_slack <ommx.PreparationPolicy.integer_slack>` | {ref}`migration-openjij-penalties` |
| `used_decision_variable_ids() (Instance, Function)` | {meth}`Instance.required_ids() <ommx.Instance.required_ids>` / {meth}`Function.required_ids() <ommx.Function.required_ids>` | {ref}`2 <migration-build>` |
| `write_mps(path)` | {meth}`save_mps(path) <ommx.Instance.save_mps>` | {ref}`5 <migration-storage>` |

For changes between individual v3 prereleases, see the [Python SDK 3.0 release notes](../release_note/ommx-3.0.md).

## Indicator trigger values

`IndicatorConstraint` and `Constraint.with_indicator` accept
`trigger_value=0` to enforce the body when the binary controlling variable is zero.
The default is `trigger_value=1`; values other than 0 and 1 are rejected. The
integer property is retained in attached, inactive, evaluated, and sampled rows
and Indicator DataFrames.

`InstanceClassClause.indicator_body_polynomial_requirements` remains a map from
`Equality` to `PolynomialRequirement`. Set
`allowed_indicator_trigger_values={0, 1}` separately to admit both trigger
values with the same body requirements. Its default is `{1}`; `{0}` admits only
the zero trigger value, and an empty set rejects all Indicators. Only 0 and 1 are valid.
An empty body map also rejects all Indicators, regardless of the trigger set.

PySCIPOpt accepts both trigger values. Big-M conversion also handles both.
The v2 wire format declares an additional required feature when any retained
Indicator has `trigger_value=0`, so readers without that feature reject the
payload instead of interpreting the trigger incorrectly.
