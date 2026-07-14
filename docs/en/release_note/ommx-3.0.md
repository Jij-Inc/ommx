# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0 contains breaking API changes. A migration guide is available in the [Python SDK v2 to v3 Migration Guide](../migration/python_sdk_v2_to_v3.md).
```

## Unreleased

Changes merged after the most recent release will be appended here as they land, and promoted to a new version section when the next release is cut.

### 🆕 Native capability declarations for HiGHS and Python-MIP adapters ([#1085](https://github.com/Jij-Inc/ommx/pull/1085))

`OMMXHighsAdapter` and `OMMXPythonMIPAdapter` now declare and enforce their
complete native translator profiles: Binary, Integer, and Continuous variables;
linear objectives; linear regular equality and inequality constraints; and both
optimization senses. Unsupported variable kinds, nonlinear functions, and
special constraints are reported through the structured
{class}`~ommx.adapter.AdapterCompatibilityError` introduced in #1084.

The migrated constructors no longer lower Indicator, OneHot, or SOS1
constraints implicitly. A direct translation attempt rejects those families
without mutating the input {class}`~ommx.Instance`; callers must perform any
intended reformulation explicitly and recheck the prepared instance.

### 🆕 Native capability declaration for the PySCIPOpt adapter ([#1086](https://github.com/Jij-Inc/ommx/pull/1086))

`OMMXPySCIPOptAdapter` now declares and enforces a coherent native profile for
Binary, Integer, and Continuous variables; quadratic objectives and regular
constraints; linear Indicator bodies; native SOS1 constraints; and both
optimization senses. The separate degree limits correctly report a quadratic
Indicator body as unsupported even though quadratic regular constraints remain
accepted.

The adapter no longer lowers OneHot constraints implicitly. Unsupported
OneHot, SemiInteger, SemiContinuous, cubic, and nonlinear-Indicator inputs are
reported through {class}`~ommx.adapter.AdapterCompatibilityError` without
mutating the input {class}`~ommx.Instance`.

### 🆕 Adapter capability profiles and compatibility reports ([#1084](https://github.com/Jij-Inc/ommx/pull/1084))

{meth}`~ommx.Instance.solver_requirements` now derives the complete active
solver-facing shape of an instance, including used variable kinds, objective
and constraint degrees, constraint relations and families, and optimization
sense. {class}`~ommx.CapabilityProfile` and
{class}`~ommx.AdapterCapabilities` describe coherent native translator
profiles and return structured {class}`~ommx.PortableCompatibilityReport`
values.

```python
from ommx import AdapterCapabilities, CapabilityProfile, DegreeLimit, Kind, Sense

binary_linear = CapabilityProfile(
    name="binary-linear",
    variable_kinds={Kind.Binary},
    objective_degree=DegreeLimit.at_most(1),
    senses={Sense.Minimize},
)
capabilities = AdapterCapabilities([binary_linear])
report = capabilities.check_compatibility(instance.solver_requirements())
```

{class}`~ommx.adapter.SolverAdapter` subclasses can declare `CAPABILITIES` and
use `check_compatibility` or `require_compatible` to combine the portable
comparison with adapter-specific preconditions without mutating the input
instance. Profiles describe direct translator input after explicit preparation;
they do not imply acceptance through relaxation or finite-penalty conversion.
`ommx.v2.Feature` and `required_features` remain separate wire-format
forward-compatibility concepts and are not adapter support declarations.

### ⚠ Legacy adapter capability vocabulary removed ([#1088](https://github.com/Jij-Inc/ommx/pull/1088))

The legacy `AdditionalCapability`, `Capabilities`, `ADDITIONAL_CAPABILITIES`,
`required_capabilities`, and `reduce_capabilities` APIs have been removed.
{class}`~ommx.SpecialConstraintKind`,
{attr}`~ommx.Instance.active_special_constraint_kinds`, and
{meth}`~ommx.Instance.lower_special_constraints` now express explicit,
direct-selection special-constraint lowering without presenting a
transformation selector as adapter support. Adapters must recheck the lowered
model against their native profile.

### 🆕 OpenJij native capability and explicit preparation ([#1087](https://github.com/Jij-Inc/ommx/pull/1087))

{class}`~ommx_openjij_adapter.OMMXOpenJijSAAdapter` now declares its native
translator profile precisely: Binary variables, an arbitrary-degree polynomial
objective, no active constraints, and minimization. Native
`check_compatibility` no longer treats Integer encoding, constraint penalties,
or sense conversion as solver support.

Pass `preparation=True` to `sample` or `solve` when the source model needs
explicit transformation. {meth}`~ommx_openjij_adapter.OMMXOpenJijSAAdapter.check_preparation`
and {meth}`~ommx_openjij_adapter.OMMXOpenJijSAAdapter.prepare` expose an audit
report whose steps distinguish exact rewrites, approximations, and finite
penalties. The returned {class}`~ommx.SampleSet` is evaluated against the source
model, and Experiment sampling continues to record that source `Instance`.

```python
check = OMMXOpenJijSAAdapter.check_preparation(
    instance,
    uniform_penalty_weight=20.0,
)
assert check.compatible
sample_set = OMMXOpenJijSAAdapter.sample(
    instance,
    preparation=True,
    uniform_penalty_weight=20.0,
)
```

The 53-bit limit checked during Integer log encoding and other OpenJij-specific
limits are adapter-owned preparation preconditions. They are neither native
capabilities nor `ommx.v2.Feature` values; `Feature` remains solely a
serialization forward-compatibility mechanism. Native Spin support will follow
after OMMX adds a Spin variable kind in [#1082](https://github.com/Jij-Inc/ommx/issues/1082).

### 🆕 `VariableIDLike` inputs for structural constraints ([#1078](https://github.com/Jij-Inc/ommx/pull/1078))

Structural-constraint construction now accepts `VariableIDLike`, defined as
`int | DecisionVariable | AttachedDecisionVariable`, wherever only a variable's
identity is required. This applies to {class}`~ommx.OneHotConstraint`,
{class}`~ommx.Sos1Constraint`, {class}`~ommx.IndicatorConstraint`, and
{meth}`Constraint.with_indicator() <ommx.Constraint.with_indicator>`. The
constraints still store OMMX variable IDs internally, and their ID getters
continue to return integers.

```python
from ommx import DecisionVariable, OneHotConstraint, Sos1Constraint

xs = [DecisionVariable.binary(i) for i in range(3)]
one_hot = OneHotConstraint(variables=xs)
sos1 = Sos1Constraint(variables=[x.id for x in xs])
indicator = (xs[0] <= 1).with_indicator(xs[1])
```

APIs that are intrinsically ID collections or mappings, such as
{meth}`~ommx.Instance.log_encode`, remain ID-based.

See [Special constraints](../user_guide/special_constraints.md) for the modeling
workflow.

### 🆕 Incremental Instance modeling ([#1077](https://github.com/Jij-Inc/ommx/pull/1077))

{class}`~ommx.Instance` can now own numeric ID assignment while a model is
built incrementally. Start with {meth}`~ommx.Instance.maximize` or
{meth}`~ommx.Instance.minimize`, create attached binary variables with
{meth}`~ommx.Instance.new_binary`, then set the objective and add constraints
directly. The existing {meth}`~ommx.Instance.from_components` workflow remains
available when components already have explicit IDs.
The ambiguous `Instance.empty()` compatibility alias is deprecated for static
type checkers; use `Instance.minimize()` instead.

```python
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
instance.add_constraint(x - y == 1, "c1")
```

`new_binary` and `add_constraint` accept the complete modeling label: `name`,
`subscripts`, `parameters`, and `description`. See the
[Instance user guide](../user_guide/instance.md) for the complete workflow.
If the maximum decision-variable ID is already `2**64 - 1`, `new_binary`
raises `ValueError` instead of propagating a Rust panic.

## 3.0.0 Beta 1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0b1-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0b1)

### ⚠ Legacy v1 `ConstraintHints` remain advisory ([#1058](https://github.com/Jij-Inc/ommx/pull/1058))

When {meth}`Instance.from_v1_bytes <ommx.Instance.from_v1_bytes>` or {meth}`ParametricInstance.from_v1_bytes <ommx.ParametricInstance.from_v1_bytes>` reads a legacy v1 payload, it now ignores `ConstraintHints` and preserves the referenced regular constraints and their context. Even a structurally plausible hint is not automatically promoted to a first-class one-hot or SOS1 constraint, so unverified metadata cannot change the feasible set or active solver requirements. Imported instances can also be serialized back to v1 because no special constraint is introduced implicitly.

Construct first-class special constraints from trusted modeling input rather than from a legacy hint alone. See the [Python SDK v2 to v3 Migration Guide](../migration/python_sdk_v2_to_v3.md) for details.

### ⚠ Dedicated Experiment artifact type ([#1033](https://github.com/Jij-Inc/ommx/pull/1033))

Committed Experiment artifacts now write `application/org.ommx.v1.experiment`
as the OCI Manifest `artifactType` instead of the generic
`application/org.ommx.v1.artifact` type. Loading an Experiment validates this
root artifact type before decoding the Experiment config, so a generic Artifact
is not interpreted as an Experiment merely because its config descriptor uses
the Experiment config media type.

This intentionally does not preserve compatibility with Experiment artifacts
created by earlier 3.0 alpha builds that wrote the generic
`application/org.ommx.v1.artifact` `artifactType`. Those alpha artifacts must be
recreated with a build that writes the dedicated Experiment artifact type.

### 🆕 Experiment Sampling records ([#1055](https://github.com/Jij-Inc/ommx/pull/1055))

{meth}`~ommx.experiment.Run.log_sample` calls a
{class}`~ommx.adapter.SamplerAdapter` and records its complete
{class}`~ommx.SampleSet` as a separate {class}`~ommx.experiment.Sampling`
record. A successful sampling call remains `finished` even when the SampleSet
contains no feasible samples. Solver calls remain {class}`~ommx.experiment.Solve`
records whose output is `Solution | None`.

```python
from ommx import SampleSet
from ommx.experiment import Experiment
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

with Experiment() as experiment:
    with experiment.run() as run:
        sample_set = run.log_sample(OMMXOpenJijSAAdapter, instance, num_reads=100)

output = experiment.runs[0].samplings[0].output
assert isinstance(output, SampleSet)
```

`Run.log_sample(..., store_diagnostics=True)` uses the same adapter diagnostics
channel as `Run.log_solve`. See the [Experiment management
tutorial](../tutorial/experiment_management.md) for the Solve and Sampling recording model.

### 🆕 Transparent attachment compression and streaming writes ([#1054](https://github.com/Jij-Inc/ommx/pull/1054))

{class}`~ommx.experiment.Experiment` and {class}`~ommx.experiment.Run` attachment
logging methods now accept `compression="zstd"`. OMMX stores the compressed
layer with a `+zstd` media-type suffix and a reserved compression annotation,
while `attachment_media_type`, `get_attachment`, typed getters, codecs, and
file export expose the original media type and decompressed payload. Readers
only decompress marked layers, so logical media types ending in `+zstd` remain
unambiguous.

```python
experiment.log_json("trace", trace_values, compression="zstd")
experiment.log_file("solver-log", log_path, compression="zstd")
```

`log_file` now streams the source file into the Local Registry instead of
buffering the whole file before the content-addressed write.

### 🆕 Local Registry ref deletion and Experiment retention ([#1053](https://github.com/Jij-Inc/ommx/pull/1053))

`ommx.artifact.remove_image()` removes a named or anonymous image ref from the
Local Registry without deleting its content-addressed blobs and returns the
atomically removed Manifest digest for rollback, or `None` when the ref did not
exist. The CLI equivalent is `ommx rm <ref>`. Its output explains that
unreferenced data remains until a separate `ommx gc --delete` removes it after
the grace period.

Deletion output includes a copyable `ommx restore-ref <ref> <manifest-digest>`
command. The equivalent Python API is `ommx.artifact.restore_image()`. Restore
validates the complete Manifest closure still present in the CAS, is serialized
against deleting GC passes, and refuses to replace a ref that has since moved
to another digest.

`ommx.artifact.prune_anonymous()` now accepts `experiments=True` to include
anonymous Experiment refs and `older_than="7d"` for age-based retention. The
CLI exposes the same behavior through `ommx prune-anonymous --experiments
--older-than 7d`. See [Experiment cleanup](../user_guide/experiment.md)
for the complete reachability and GC workflow.

```python
from ommx.artifact import prune_anonymous, remove_image, restore_image

removed_digest = remove_image("example.com/team/experiment:obsolete")
assert removed_digest is not None
restore_image("example.com/team/experiment:obsolete", removed_digest)
prune_anonymous(delete=True, experiments=True, older_than="7d")
```

### 🆕 Configurable Experiment autosave frequency ([#1052](https://github.com/Jij-Inc/ommx/pull/1052))

{class}`~ommx.experiment.Experiment` can now batch, rate-limit, or disable the
rolling draft checkpoints written after Runs close. The default remains one
checkpoint per closed Run. Autosave policies belong to the current unsealed
session and do not disable failed or interrupted checkpoints produced when an
Experiment context exits exceptionally.

```python
from ommx.experiment import AutosavePolicy, Experiment

experiment = Experiment("example.com/team/sweep:latest")
experiment.set_autosave_policy(AutosavePolicy.every_n_runs(25))
```

Use `AutosavePolicy.min_interval(seconds)` for time-based rate limiting or
`AutosavePolicy.disabled()` when Run-close recovery checkpoints are not needed.
See [Experiment Discovery, Recovery, and Cleanup](../user_guide/experiment.md)
for the recovery and storage tradeoffs.

### 🆕 Artifact and Experiment listing from the Local Registry ([#1029](https://github.com/Jij-Inc/ommx/pull/1029))

`ommx.artifact.list_artifacts()` now lists every OMMX Artifact ref from the
SQLite Local Registry. The returned `ArtifactRef` records include the image
name, Manifest and Config digests, update timestamp, `artifactType`, Manifest
annotations, and the complete OCI Manifest as a Python `dict`.

`ommx.experiment.list_experiments()` provides the Experiment-specific view. Its
`ExperimentRef` records additionally include status, run/solve counts, and the
complete Experiment Config. Both functions accept an optional `prefix` filter
matched against the full image reference string.

Internal Experiment checkpoint refs are hidden from `list_artifacts()` by
default. `ommx.experiment.list_experiment_checkpoints()` provides the recovery
view, filtering by the original requested image-name prefix and any combination
of `draft`, `failed`, and `interrupted` status. Pass
`list_artifacts(..., include_internal=True)` only when diagnosing the underlying
registry refs.

Manifest and Experiment Config JSON are cached in SQLite under their content
digests. A missing row is backfilled from the CAS on listing; subsequent
listings do not need to construct each Experiment. Existing version 1 Local
Registries are migrated to version 2 in place while preserving refs and the
registry ID. Invalid per-ref cache entries are repaired from the CAS when
possible and otherwise skipped with `RuntimeWarning`. Malformed individual ref
identities are also warned and skipped; `strict=True` turns these individual
failures into errors. SQLite schema, query, and cache-write failures remain hard
errors.

Experiments can also store caller-owned manifest annotations with
`Experiment.set_annotation(...)`; OMMX-reserved annotation keys remain rejected.

```python
from ommx.artifact import list_artifacts
from ommx.experiment import Experiment, list_experiment_checkpoints, list_experiments

with Experiment("example.com/team/experiments/demo:latest") as experiment:
    experiment.set_annotation("com.example.problem", "demo")

refs = list_experiments("example.com/team/experiments")
assert refs[0].annotations["com.example.problem"] == "demo"
assert refs[0].config["status"] == "finished"

artifacts = list_artifacts("example.com/team")
assert artifacts[0].manifest["artifactType"].startswith("application/org.ommx")

recoverable = list_experiment_checkpoints(
    "example.com/team/experiments",
    statuses=["draft", "failed", "interrupted"],
)
```

Local Registry refs now store only their target manifest digest. Consequently,
`AnonymousArtifactRef.size` and `AnonymousArtifactRef.media_type` are removed;
descriptor fields are no longer part of the ref listing API.

### 🆕 Non-finite float Run parameters ([#1043](https://github.com/Jij-Inc/ommx/pull/1043))

{meth}`~ommx.experiment.Run.log_parameter` now accepts `float("inf")`,
`-float("inf")`, and `float("nan")`. These values round-trip through committed
Experiment artifacts and are restored by
{meth}`~ommx.experiment.Experiment.run_parameters_df` with pandas nullable
dtypes. This keeps legitimate experiment observations such as unbounded ratios
or infeasibility summaries distinct from missing cells: logged `NaN` remains a
float `NaN`, while missing float cells are represented as pandas `NA`.

The run-parameter table layer is stored as MessagePack instead of JSON so it
can preserve IEEE 754 non-finite values. Individual NaN payload bits are not
part of the API guarantee.

### 🆕 Unary integer encoding ([#1010](https://github.com/Jij-Inc/ommx/pull/1010))

{meth}`~ommx.Instance.unary_encode` is now available as a sampler-friendly
alternative to {meth}`~ommx.Instance.log_encode` for finite integer variables.
For an integer variable `x` in `[lower, upper]`, unary encoding introduces
`upper - lower` binary variables and substitutes `x = lower + sum(b)`.

Every binary assignment decodes to a value in the original integer range, so
no encoding-validity constraint or penalty is added. Since the number of
auxiliary variables grows linearly with the range width, prefer this encoding
for narrow ranges and keep using log encoding for wider variables. To avoid
accidental large allocations, `Instance.unary_encode()` rejects variables with
range width above `max_range` (default: `16`); pass a larger `max_range`
explicitly when the auxiliary-variable cost is intentional.

Both `Instance.unary_encode(..., atol=...)` and `Instance.log_encode(..., atol=...)`
use the same ATol-aware integer-bound normalization as the rest of the SDK.
`Instance.log_encode()` rejects integer ranges that would require more than 53
auxiliary binary variables instead of accepting impractically large encoded
search spaces.
Both encoders also reject non-point integer ranges outside the unit-spaced
`float` integer interval, so every accepted encoded integer value remains
distinguishable after adding the encoding offset.
Passing an explicit fixed decision-variable ID is rejected before substitution,
so fixed values and dependent-variable assignments remain disjoint.

```python
from ommx import DecisionVariable, Instance

x = DecisionVariable.integer(0, lower=2, upper=5)
instance = Instance.from_components(
    sense=Instance.MAXIMIZE,
    objective=x,
    decision_variables=[x],
    constraints={},
)

instance.unary_encode({0})
```

### 🆕 Context-aware function formatting ([#1004](https://github.com/Jij-Inc/ommx/pull/1004), [#1011](https://github.com/Jij-Inc/ommx/pull/1011))

{class}`~ommx.Instance` and {class}`~ommx.ParametricInstance` now provide
{meth}`~ommx.Instance.format_function` / {meth}`~ommx.ParametricInstance.format_function`
for rendering a function with decision-variable and parameter modeling labels.
The context-free {class}`~ommx.Function` text representation remains raw-ID
based.

Calling {func}`str` / {func}`repr` on {class}`~ommx.Instance` or
{class}`~ommx.ParametricInstance` now prints a compact summary with
context-aware objective, constraint, and named-function expressions. This makes
`print(instance)` useful for checking that encoded IDs still correspond to the
modeling labels imported from upstream modeling tools.

For notebook previews, use {meth}`~ommx.Instance.display_function` or
{meth}`~ommx.ParametricInstance.display_function`. They return
{class}`ommx.display.FunctionDisplay`, which keeps truncation metadata and
renders escaped HTML in Jupyter.

```python
from ommx import DecisionVariable, Instance

x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
instance = Instance.from_components(
    sense=Instance.MINIMIZE,
    objective=x[0] + 2 * x[1],
    decision_variables=x,
    constraints={},
)

assert instance.format_function(instance.objective) == "x[0] + 2*x[1]"
preview = instance.display_function(instance.objective)
```

## 3.0.0 Alpha 8

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a8-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a8)

### ⚠ Top-level `ommx` is the public Python SDK namespace ([#979](https://github.com/Jij-Inc/ommx/pull/979))

SDK domain classes are now imported from top-level `ommx`, not from `ommx.v1`. The internal PyO3 extension remains `ommx._ommx_rust`, but users and adapters should treat top-level `ommx` as the public API surface.

```python
from ommx import Instance, DecisionVariable, Function, Solution
```

`ommx.v1` is no longer the Python SDK object namespace. It is reserved for protobuf wire-format concepts such as schema/package names and media types, and importing SDK domain classes from `ommx.v1` now raises a migration error. See the [Python SDK v2 to v3 Migration Guide](../migration/python_sdk_v2_to_v3.md) for the broader import migration.

### ⚠ Constraint metadata setter names ([#975](https://github.com/Jij-Inc/ommx/pull/975))

Constraint metadata replacement now consistently uses the `set_*` prefix. `Constraint.add_name`, `Constraint.add_description`, and the same scalar replacing aliases on `AttachedX` handles are removed. Use `set_name` and `set_description` instead.

`add_parameters` now merges the provided entries into the existing parameter map, matching `add_parameter` and `add_subscripts`. Use `set_parameters` when replacing the whole map.

### ⚠ Protobuf-backed annotations and read-only annotation views ([#939](https://github.com/Jij-Inc/ommx/pull/939))

Annotations on {class}`~ommx.Instance`, {class}`~ommx.ParametricInstance`, {class}`~ommx.Solution`, and {class}`~ommx.SampleSet` are now stored in the protobuf payload instead of living only in Python-side wrapper state or Artifact descriptors. `to_v1_bytes()` / `from_v1_bytes()` and `to_v2_bytes()` / `from_v2_bytes()` therefore preserve titles, licenses, solver metadata, and user extension annotations. When reading older Artifacts, descriptor-only annotations are still merged in, with protobuf metadata taking precedence if both locations define the same OMMX key.

The `annotations` property is now a read-only `types.MappingProxyType[str, str]` projection. Mutating `obj.annotations[...]` or assigning `obj.annotations = {...}` now raises an error; update OMMX metadata through dedicated properties and update user annotations with `add_user_annotation`, `add_user_annotations`, or `replace_annotations`.

```python
from ommx import Instance

instance = Instance.minimize()
instance.title = "portfolio"
instance.add_user_annotation("owner", "analytics")

restored = Instance.from_v1_bytes(instance.to_v1_bytes())
assert restored.title == "portfolio"
assert restored.get_user_annotation("owner") == "analytics"
```

{class}`~ommx.Solution` and {class}`~ommx.SampleSet` also expose process metadata through `instance`, `solver`, `parameters`, `start`, and `end`; those fields round-trip through both protobuf bytes and Artifacts.

### 🆕 `Instance.populate_state` for complete solver states ([#944](https://github.com/Jij-Inc/ommx/pull/944))

{meth}`~ommx.Instance.populate_state` is now exposed in the Python SDK. It validates a partial solver state against an Instance and returns a {class}`~ommx.State` containing every decision variable by filling fixed variables, irrelevant variables, and dependent variables owned by the Instance.

```python
from ommx import DecisionVariable, Instance

x = {i: DecisionVariable.continuous(i) for i in [1, 2, 5, 10, 99]}
instance = Instance.from_components(
    decision_variables=list(x.values()),
    objective=x[1] + x[2],
    constraints={},
    sense=Instance.MINIMIZE,
)
instance.substitute({10: x[1] + x[2], 5: x[10] + 1})
instance = instance.partial_evaluate({99: 4.0})

state = instance.populate_state({1: 2.0, 2: 3.0})
assert state.entries == {1: 2.0, 2: 3.0, 5: 6.0, 10: 5.0, 99: 4.0}
```

### ⚠ Decision variable role queries on `Instance` ([#946](https://github.com/Jij-Inc/ommx/pull/946))

The Python SDK no longer exposes `DecisionVariableUsage` or `DecisionVariableUsageEntry` objects. Use {attr}`~ommx.Instance.used_decision_variables` when adapters need the solver input variables, and use {meth}`~ommx.Instance.decision_variable_role`, {meth}`~ommx.Instance.decision_variable_roles`, {meth}`~ommx.Instance.fixed_decision_variables`, {meth}`~ommx.Instance.dependent_decision_variable_ids`, and {meth}`~ommx.Instance.irrelevant_decision_variable_ids` to query state roles directly from the owning Instance.

{meth}`~ommx.Instance.decision_variables_df` continues to include the `state_role` column, so DataFrame-based workflows can inspect `used`, `fixed`, `dependent`, and `irrelevant` roles without constructing a separate usage object.

### ⚠ Fixed decision-variable values are owned by instances ([#959](https://github.com/Jij-Inc/ommx/pull/959))

Fixed decision-variable values are now owned by {class}`~ommx.Instance` / {class}`~ommx.ParametricInstance` instead of detached {class}`~ommx.DecisionVariable` objects. A detached {class}`~ommx.DecisionVariable` remains a modeling snapshot for the variable definition and label, but it no longer carries owner-side fixed-value state, so `DecisionVariable.substituted_value` is no longer available.

Use {meth}`~ommx.Instance.fixed_decision_variables` to inspect all fixed values, or `instance.attached_decision_variable(id).substituted_value` when you need the value through a variable handle. {meth}`~ommx.Instance.decision_variables_df` continues to include the `substituted_value` column, populated from the owning instance.

### 🛠 Coefficient arithmetic errors are reported through Python `ValueError` ([#953](https://github.com/Jij-Inc/ommx/pull/953))

Python expression construction and comparisons now propagate coefficient arithmetic errors as `ValueError` instead of relying on infallible Rust operators. Operations that would create non-finite coefficients, such as overflow during addition or multiplication, now fail with messages like `Coefficient must be finite`. Arithmetic cancellation and underflow-to-zero remove the affected term instead of storing an invalid zero coefficient.

### 🆕 Adapter diagnostics progress histories for HiGHS and PySCIPOpt ([#945](https://github.com/Jij-Inc/ommx/pull/945), [#948](https://github.com/Jij-Inc/ommx/pull/948))

The HiGHS Adapter now records MIP progress snapshots from the HiGHS logging callback and records a termination report before decoding, so final status, MIP bounds, gap, feasibility summaries, runtime, and version metadata remain available even when decoding raises. The new {class}`~ommx_highs_adapter.HighsDiagnosticsAnalyzer` can analyze either typed diagnostics collected during a direct solve or dictionaries loaded from an Experiment.

PySCIPOpt progress histories now include a synthetic `TERMINATION` row when diagnostics include a termination report, so `progress_history_records` and `progress_history_df` include the final solver state without duplicating the separate termination report.

See [Adapter-specific Diagnostics](../user_guide/adapter_diagnostics.md) for the direct and Experiment-based workflows.

### 🆕 Versioned protobuf bytes APIs for top-level roots ([#989](https://github.com/Jij-Inc/ommx/pull/989))

{class}`~ommx.Instance`, {class}`~ommx.ParametricInstance`, {class}`~ommx.Solution`, and {class}`~ommx.SampleSet` now expose explicit versioned protobuf bytes APIs. Use `to_v1_bytes()` / `from_v1_bytes(...)` for the legacy `ommx.v1` protobuf roots, and `to_v2_bytes()` / `from_v2_bytes(...)` for the new `ommx.v2` protobuf roots. Use the v2 methods when exchanging data that contains first-class indicator, one-hot, or SOS1 constraints.

The old unversioned `to_bytes()` / `from_bytes(...)` methods on these top-level roots are removed. Replace them with `to_v1_bytes()` / `from_v1_bytes(...)` when you need the legacy v1 wire format, or with the v2 methods for normalized v2 payloads.

The v1-only DTOs {class}`~ommx.State`, {class}`~ommx.Samples`, and {class}`~ommx.Parameters` also use `to_v1_bytes()` / `from_v1_bytes(...)` so Python byte APIs always name the protobuf version they target.

Artifact and Experiment solve payloads now store these top-level roots as `ommx.v2` payloads, while still reading existing `ommx.v1` payload layers for older Artifacts.

## 3.0.0 Alpha 7

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a7-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a7)

### 🆕 Manual `solver_input` workflows in Experiment records ([#934](https://github.com/Jij-Inc/ommx/pull/934))

{meth}`~ommx.experiment.Run.open_solve` now opens a manual Solve scope for advanced solver features that are not covered by the Adapter API. Inside the scope, use `solve.solver_input` to operate the backend solver model directly, run the backend optimizer, then call `solve.decode(...)` so the decoded {class}`~ommx.Solution` becomes the Experiment Solve output. Manual adapter options can be recorded with `solve.log_adapter_option(...)`, and `store_diagnostics=True` stores diagnostics recorded through `solve.diagnostics` until the scope exits. After the scope closes, {attr}`~ommx.experiment.OpenSolve.terminal_state` exposes the final outcome plus trace and diagnostics finalization state for advanced debugging.

See the [Experiment management tutorial](../tutorial/experiment_management.md) for the workflow example.

## 3.0.0 Alpha 6

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a6-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a6)

### 🆕 Adapter-specific solve diagnostics ([#913](https://github.com/Jij-Inc/ommx/pull/913))

Solver adapters now have an adapter-specific diagnostics channel for preserving backend solver information that does not belong in the common {class}`~ommx.Solution` result. Direct adapter calls can pass {class}`~ommx.adapter.DiagnosticCollector` to {meth}`~ommx.adapter.SolverAdapter.solve` through the reserved `diagnostics` keyword, while {meth}`~ommx.experiment.Run.log_solve` owns that keyword and stores recorded diagnostics with each Experiment {class}`~ommx.experiment.Solve` when called with `store_diagnostics=True`. Experiment diagnostics are disabled by default so adapter-side collection overhead is opt-in.

The PySCIPOpt Adapter now emits {class}`~ommx_pyscipopt_adapter.SCIPProgressSnapshot` diagnostics from SCIP `BESTSOLFOUND` and `DUALBOUNDIMPROVED` callbacks, plus {class}`~ommx_pyscipopt_adapter.SCIPTerminationReport` after `model.optimize()`. The termination report includes SCIP status, primal/dual bounds, gap, incumbent objective value, node counts, LP/cut/solution counters, primal-dual integral, timing, and SCIP/PySCIPOpt version metadata. {class}`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` can post-process the typed collector contents or dictionaries loaded from an Experiment into records or pandas DataFrames. With direct collection, the termination report is recorded before decoding back to an OMMX Solution, so it remains available to the caller even when decoding raises an adapter exception such as infeasible or unbounded detection.

See [Adapter-specific Diagnostics](../user_guide/adapter_diagnostics.md) for the full API workflow and the PySCIPOpt report field references.

## 3.0.0 Alpha 5

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a5-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a5)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### 🆕 Run-scoped Experiment trace storage ([#910](https://github.com/Jij-Inc/ommx/pull/910), [#916](https://github.com/Jij-Inc/ommx/pull/916))

{class}`~ommx.experiment.Experiment`, {meth}`~ommx.experiment.Experiment.with_temp_local_registry`, and {meth}`~ommx.experiment.Experiment.fork` now accept `store_trace=True`. When enabled, each `with experiment.run()` context captures the OpenTelemetry spans emitted inside that Run and stores one trace on the closed {class}`~ommx.experiment.SealedRun`. The stored trace is returned as {class}`~ommx.tracing.TraceResult` from {attr}`~ommx.experiment.SealedRun.trace`, and is carried through commit, load, and fork.

See [Tracing and Profiling](../user_guide/tracing.ipynb) for the full tracing workflow, renderers, and OpenTelemetry setup notes.

```python
from ommx.experiment import Experiment
from ommx.tracing import render_text_tree
from ommx_highs_adapter import OMMXHighsAdapter

with Experiment.with_temp_local_registry(store_trace=True) as experiment:
    with experiment.run() as run:
        run.log_solve(OMMXHighsAdapter, instance)

loaded = Experiment.from_artifact(experiment.artifact)
trace = loaded.runs[0].trace
if trace is not None:
    print(render_text_tree(trace))
```

The stored payload is OTLP protobuf, so {class}`~ommx.tracing.TraceResult` now owns the exported request, exposes flattened `spans`, and can round-trip with `otlp_protobuf()` / `from_otlp_protobuf()`. Text and Chrome trace renderers also use domain-oriented span names such as `Run`, `solve`, `convert`, `call`, and `decode`, and surface instrumentation scope while hiding debug-only source attributes.

### ⚠ Experiment attachments are now name-indexed ([#924](https://github.com/Jij-Inc/ommx/pull/924))

Experiment and Run attachments are now stored as name-indexed tables in the Experiment config. The public Python API is name-oriented: use `attachment_names`, `attachment_media_type(name)`, `get_attachment(name)`, the typed getters such as `get_json(name)` and `get_instance(name)`, `get_blob(name)`, `get_with_codec(...)`, and `write_attachment(...)`.

```python
loaded = Experiment.from_artifact(experiment.artifact)

for name in loaded.attachment_names:
    print(name, loaded.attachment_media_type(name))
    value = loaded.get_attachment(name)
```

Descriptor-oriented attachment views from earlier 3.0 alphas, including `Experiment.experiment_attachments` and `SealedRun.attachments`, are removed. Registry-backed descriptors remain internal so attachment names, media types, file export names, and checkpoint metadata stay in the Experiment config instead of descriptor annotations.

### 🆕 Experiment checkpoints and restore from interrupted sessions ([#917](https://github.com/Jij-Inc/ommx/pull/917))

{class}`~ommx.experiment.Experiment` now publishes local checkpoints for partial experiment state. Closing a {class}`~ommx.experiment.Run` writes a best-effort draft checkpoint, and exiting an Experiment with an exception writes a failed or interrupted checkpoint instead of advancing the successful Experiment image reference. Closed Runs keep their attachments, solves, traces, and run parameters, including Runs closed as `"failed"` or `"interrupted"` after exceptions such as `KeyboardInterrupt`.

See [Experiment Discovery, Recovery, and Cleanup](../user_guide/experiment.md) for Experiment catalog filtering, Run close boundaries, checkpoint restoration, and Local Registry cleanup behavior.

Use {meth}`~ommx.experiment.Experiment.restore_from_checkpoint` with the original Experiment image name to resume from the latest checkpoint:

```python
from ommx.experiment import Experiment

image_name = "ghcr.io/example/team/experiment:notebook"

try:
    with Experiment(image_name) as experiment:
        with experiment.run() as run:
            run.log_parameter("solver", "highs")
            raise KeyboardInterrupt
except KeyboardInterrupt:
    pass

experiment = Experiment.restore_from_checkpoint(image_name)
assert experiment.image_name == image_name
```

Successful `commit()` still publishes only the requested image reference and removes the local checkpoint when present. Checkpoint Artifact handles and checkpoint image names are intentionally not exposed in the Python API; users restore by remembering the original Experiment image name.

### 🆕 Local Registry cleanup ([#919](https://github.com/Jij-Inc/ommx/pull/919))

The `ommx` CLI now provides Local Registry maintenance commands for the SQLite-backed Artifact registry. Use `ommx gc` to report blobs that are unreachable from SQLite refs, including Experiment checkpoint refs. The command protects recently written unreachable blobs with a grace period so active Experiment writes are not deleted accidentally.

Destructive cleanup commands report by default and mutate the registry only when `--delete` is passed:

```bash
ommx prune-anonymous
ommx gc
ommx prune-anonymous --delete
ommx gc --delete
```

Normal reports show counts and sizes rather than raw digests. Pass `--show-digests` when low-level diagnostics are needed.

The same cleanup operations are also exposed from the Python SDK as
{func}`ommx.artifact.prune_anonymous` and {func}`ommx.artifact.gc`. These
functions are report-only by default, mutate the registry with `delete=True`,
and return structured report objects for notebook and script use.

### 🆕 Typed attachment codecs for Experiments ([#921](https://github.com/Jij-Inc/ommx/pull/921))

The new {class}`ommx.experiment.attachments.AttachmentCodec` protocol lets packages that own Python payload types define how those values are stored as Experiment attachments. A codec class provides a media type plus `encode` / `decode` methods, and OMMX calls it through `log_with_codec` and `get_with_codec` on both Experiment-level and Run-level attachments.

See the {ref}`Attachable Data Formats <experiment-management-attachable-data-formats>` section of the Experiment management tutorial for a JijModeling `Problem` codec example.

```python
from ommx.experiment import Experiment


class TextCodec:
    media_type = "text/plain"

    @staticmethod
    def encode(value: str) -> bytes:
        return value.encode()

    @staticmethod
    def decode(data: bytes) -> str:
        return data.decode()


with Experiment.with_temp_local_registry() as experiment:
    experiment.log_with_codec(TextCodec, "note", "created outside OMMX")

loaded = Experiment.from_artifact(experiment.artifact)
assert loaded.get_with_codec(TextCodec, "note") == "created outside OMMX"
```

The stored attachment media type is validated before decoding, so using the wrong codec for an attachment fails before the codec's `decode` method is called.

### 🆕 File attachments for Experiments ([#922](https://github.com/Jij-Inc/ommx/pull/922))

{class}`~ommx.experiment.Experiment` and {class}`~ommx.experiment.Run` can now attach files that were produced outside OMMX. Use `log_file` to copy an existing file into the Experiment Artifact. OMMX stores the file bytes as an attachment blob, records the original basename for later export, and uses an explicitly provided media type or Rust SDK content-based inference with an `application/octet-stream` fallback.

Committed experiment and run views now also provide `write_attachment` to restore an attachment blob back to disk. For libraries that accept a binary file-like object, wrap the existing `get_blob` result with `io.BytesIO`.

```python
import io
from pathlib import Path

from ommx.experiment import Experiment

with Experiment.with_temp_local_registry() as experiment:
    experiment.log_file("input-spreadsheet", "input.xlsx")

loaded = Experiment.from_artifact(experiment.artifact)
spreadsheet_file = io.BytesIO(loaded.get_blob("input-spreadsheet"))
Path("restored").mkdir(parents=True, exist_ok=True)
loaded.write_attachment("input-spreadsheet", "restored/input.xlsx")
```

## 3.0.0 Alpha 4

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a4-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a4)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### ⚠ SQLite-based Local Registry ([#871](https://github.com/Jij-Inc/ommx/pull/871), [#872](https://github.com/Jij-Inc/ommx/pull/872))

In v3, local Artifact storage is organized around the SQLite-based Local Registry. Artifact blobs are stored in content-addressed storage, while image-name references and registry metadata are managed in SQLite. APIs that depended on the old disk OCI directory cache are removed; the user-facing flow is now to commit an Artifact into the Local Registry, then `save` / `push` / `load` that committed Artifact.

Alongside this storage model and the new `Experiment` API, the old `ArtifactBuilder` is reshaped as {class}`~ommx.artifact.ArtifactDraft`. An `ArtifactDraft` represents an uncommitted Artifact draft; after it is committed to the Local Registry, the resulting {class}`~ommx.artifact.Artifact` can be saved or pushed. `.ommx` archives are import/export exchange formats for the Local Registry. The main breaking changes are:

- `ArtifactBuilder.new_archive` → {func}`ArtifactDraft.new <ommx.artifact.ArtifactDraft.new>` + {func}`Artifact.save <ommx.artifact.Artifact.save>` (new method).
- `ArtifactBuilder.new_archive_unnamed` → {func}`ArtifactDraft.new_anonymous <ommx.artifact.ArtifactDraft.new_anonymous>` + `Artifact.save(path)`. In v2, an unnamed archive literally had no image name and was read back as `None`. In v3, an anonymous Artifact gets an automatically generated `<registry-id8>.ommx.local/anonymous:<timestamp>-<nonce>` image name from the Local Registry, so it can still be saved, loaded again, and cleaned up.
- {func}`Artifact.load_archive <ommx.artifact.Artifact.load_archive>` raises a migration error pointing at the two replacement methods: {func}`Artifact.import_archive <ommx.artifact.Artifact.import_archive>` (imports the archive into the user's persistent SQLite Local Registry — the v3 successor with registry-write semantics) and {func}`Artifact.inspect_archive <ommx.artifact.Artifact.inspect_archive>` (side-effect-free read of the manifest + layer descriptors, returns a new {class}`ArchiveManifest <ommx.artifact.ArchiveManifest>` view). v2's `load_archive` opened archives in place with no registry side effect, so the rename makes the semantic shift explicit instead of silently writing into the registry on upgrade. `import_archive` accepts v2 archives produced by `ArtifactBuilder.new_archive_unnamed` (no `org.opencontainers.image.ref.name` annotation) by synthesizing an anonymous name on the fly; `inspect_archive` reads such archives back with `ArchiveManifest.image_name = None` (no registry context for synthesis).
- CLI `ommx push <archive>` and `ommx push <oci-dir>` removed — load into the registry first, then push by image name.
- New CLI `ommx prune-anonymous [--delete]` reports accumulated anonymous-commit entries by default and removes them only when `--delete` is passed.
- `ommx.get_image_dir(...)` and the CLI `ommx image-dir <name>` subcommand are removed. The return value was a v2 disk-cache path (`<root>/<image_name>/<tag>/`) that no longer corresponds to any v3 storage location — the SQLite Local Registry stores blobs content-addressed and refs in SQLite — so pointing users at it was actively misleading. Existing v2 caches still migrate via `ommx import-legacy`.

See the {ref}`Python SDK v2 to v3 Migration Guide §13 <13-artifact-api-archive-becomes-an-exchange-format>` for the full before/after code and migration checklist.

### 🆕 Artifact-backed experiment management API: `ommx.experiment` ([#882](https://github.com/Jij-Inc/ommx/pull/882), [#885](https://github.com/Jij-Inc/ommx/pull/885), [#886](https://github.com/Jij-Inc/ommx/pull/886), [#903](https://github.com/Jij-Inc/ommx/pull/903))

The new `ommx.experiment` module records experiment inputs, run conditions, and Solver/Sampler results as one OMMX Artifact. Use {class}`~ommx.experiment.Experiment`, {class}`~ommx.experiment.Run`, and {class}`~ommx.experiment.Solve` to store per-run comparison parameters, attachments, and solve input/output data in the Local Registry.

See the [Experiment management tutorial](../tutorial/experiment_management.md) for the basic workflow, sharing an Experiment, loading a committed Experiment, and creating derived experiments with fork.

### 🆕 `Run.log_solve` records solve input/output and adapter options ([#902](https://github.com/Jij-Inc/ommx/pull/902))

{meth}`~ommx.experiment.Run.log_solve` is now available. Pass a subclass of `ommx.adapter.SolverAdapter` and an {class}`~ommx.Instance`; OMMX calls the adapter's `solve`, then stores the input Instance, output Solution, adapter class name, and JSON-serializable keyword arguments as a {class}`~ommx.experiment.Solve`.

```python
from ommx.experiment import Experiment
from ommx_highs_adapter import OMMXHighsAdapter
from ommx import Instance, Solution

with Experiment() as experiment:
    with experiment.run() as run:
        solution = run.log_solve(OMMXHighsAdapter, instance, verbose=False)
        run.log_parameter("objective", solution.objective)

solve = experiment.runs[0].solves[0]
assert solve.adapter.endswith("OMMXHighsAdapter")
assert isinstance(solve.input, Instance)
output = solve.output
assert isinstance(output, Solution)
assert output.feasible
assert solve.adapter_options == {"verbose": False}
```

Adapter options are solve-scoped metadata, so they do not appear in {meth}`~ommx.experiment.Experiment.run_parameters_df`, which is the table for comparing runs. Record values explicitly with {meth}`~ommx.experiment.Run.log_parameter` when you want them in that DataFrame.

### 🆕 Experiment fork and lineage ([#905](https://github.com/Jij-Inc/ommx/pull/905))

{meth}`~ommx.experiment.Experiment.fork` starts a new uncommitted Experiment from a committed one. The child inherits the parent's attachments, Runs, Solves, Samplings, and Run parameters, while the parent remains unchanged. When the child is committed after adding new Runs or attachments, the parent manifest descriptor is recorded as the OCI `subject`.

```python
from ommx.experiment import Experiment
from ommx_highs_adapter import OMMXHighsAdapter

loaded = Experiment.load("ghcr.io/jij-inc/ommx/tutorial/experiment:baseline")

with loaded.fork("ghcr.io/jij-inc/ommx/tutorial/experiment:capacity-64") as child:
    with child.run() as run:
        run.log_parameter("capacity", 64)
        run.log_solve(OMMXHighsAdapter, instance, verbose=False)
```

Forking creates a new Artifact Manifest, but Instance / Solution / attachment payloads continue to reference content-addressed blobs in the Local Registry, so the data bodies are not duplicated. Saving or pushing the fork shares the complete forked Experiment, including Runs and Solves inherited from the parent.

### 🆕 `Instance.substitute` / `ParametricInstance.substitute` ([#891](https://github.com/Jij-Inc/ommx/pull/891), [#897](https://github.com/Jij-Inc/ommx/pull/897))

{meth}`~ommx.Instance.substitute` and {meth}`~ommx.ParametricInstance.substitute` are now available from Python. Pass a dictionary from decision-variable IDs to replacement {class}`~ommx.Function` expressions; OMMX rewrites those variables in the objective and active constraints in-place. This exposes the general substitution mechanism behind `log_encode`, so users can implement custom variable transformations such as unary or one-hot encodings.

```python
from ommx import DecisionVariable, Instance

x = DecisionVariable.integer(0, lower=0, upper=3)
b = [DecisionVariable.binary(i) for i in (1, 2)]
instance = Instance.from_components(
    decision_variables=[x, *b],
    objective=x,
    constraints={},
    sense=Instance.MAXIMIZE,
)

instance.substitute({0: b[0] + 2 * b[1]})
assert str(instance.objective) == "Function(x1 + 2*x2)"
```

This API is an algebraic rewrite. It does not translate the substituted variable's `kind` / `lower` / `upper` into constraints on the replacement expression. To preserve the optimization problem, use a domain-preserving encoding or add the required linking / bound constraints yourself. `ParametricInstance.substitute` may leave parameters in replacement expressions, so symbolic variable transformations can be applied before concrete values are supplied with `with_parameters`.

## 3.0.0 Alpha 3

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a3-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a3)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### ⚠ `*_df` accessors are methods + `include=` filter + sidecar DataFrames ([#846](https://github.com/Jij-Inc/ommx/pull/846))

Every `*_df` accessor on `Instance` / `ParametricInstance` / `Solution` / `SampleSet` is now a regular method instead of a `#[getter]` property. Existing call sites need parentheses:

```python
# Before
df = solution.constraints_df

# After
df = solution.constraints_df()
```

The wide `*_df` methods take an `include` argument that gates the label / parameters column families. The default `include=("label", "parameters")` preserves the v2-equivalent wide shape:

```python
solution.decision_variables_df()                       # core + label + parameters
solution.decision_variables_df(include=[])             # core only
solution.decision_variables_df(include=["label"])      # core + label
solution.decision_variables_df(include=["parameters"]) # core + parameters
```

Six new long-format / id-indexed sidecar accessors read directly from the SoA label/context stores. `kind=` selects the constraint family (`"regular"` / `"indicator"` / `"one_hot"` / `"sos1"`, default `"regular"`):

- `constraint_context_df(kind=...)` — id-indexed (`name` / `subscripts` / `description`)
- `constraint_parameters_df(kind=...)` — long format (`{kind}_constraint_id` / `key` / `value`)
- `constraint_provenance_df(kind=...)` — long format (`{kind}_constraint_id` / `step` / `source_kind` / `source_id`)
- `constraint_removed_reasons_df(kind=...)` — long format (`{kind}_constraint_id` / `reason` / `key` / `value`)
- `variable_labels_df()` — id-indexed
- `variable_parameters_df()` — long format

Sidecar index names are kind-qualified (`regular_constraint_id` / `indicator_constraint_id` / `one_hot_constraint_id` / `sos1_constraint_id` / `variable_id`) so accidental cross-id-space `df.join()` mistakes surface in `df.head()` and friends. Long-format `*_parameters_df` / `*_removed_reasons_df` rows are sorted by `(id, key)`, and empty long-format DataFrames keep their column schema instead of returning a column-less frame.

### ⚠ `removed_reason` column gated by `include=` ([#796](https://github.com/Jij-Inc/ommx/pull/796), [#847](https://github.com/Jij-Inc/ommx/pull/847))

In v2.5.1 {meth}`Solution.constraints_df <ommx.Solution.constraints_df>` carried a `removed_reason` column unconditionally. The initial `include=` gate of that column landed in 3.0.0a2 (#796), and 3.0.0a3 finalizes it into the `kind=` / `include=` / `removed=` dispatch shape documented above (#847): the column is opted in by `"removed_reason"` in `include=` (a unit flag that controls both the reason name and `removed_reason.{key}` parameter columns). Rows whose constraint was not removed before evaluation get NA in those columns.

```python
# Before (2.5.1)
df = solution.constraints_df  # contains a 'removed_reason' column

# After (3.0.0a3 — `*_df` are now methods)
df = solution.constraints_df()  # no removed_reason column
df = solution.constraints_df(include=("label", "parameters", "removed_reason"))
# ↳ adds removed_reason / removed_reason.{key} (NA for active rows)
```

The same `kind=` / `include=` shape applies on {class}`~ommx.SampleSet`. On {class}`~ommx.Instance` and {class}`~ommx.ParametricInstance`, `removed=True` returns active + removed rows in one DataFrame and auto-sets `"removed_reason"` so removed rows are distinguishable.

### ⚠ `to_bytes` / `from_bytes` removed from non-top-level types ([#845](https://github.com/Jij-Inc/ommx/pull/845))

Bytes serialization is removed from the following component-level types:

- {class}`~ommx.Function`, {class}`~ommx.Linear`, {class}`~ommx.Quadratic`, {class}`~ommx.Polynomial`
- {class}`~ommx.Parameter`
- {class}`~ommx.NamedFunction`, {class}`~ommx.EvaluatedNamedFunction`, {class}`~ommx.SampledNamedFunction`
- {class}`~ommx.DecisionVariable`, {class}`~ommx.EvaluatedDecisionVariable`, {class}`~ommx.SampledDecisionVariable`

These methods originally existed to ferry values across the Python ↔ Rust boundary back when the Python SDK had its own protobuf-based wrapper layer and had to serialize on every hop. With the v3 transition to direct PyO3 re-exports the boundary disappears, so element-level bytes round-trips no longer serve a purpose, and keeping them aligned with the label/context storage redesign would only add maintenance cost. Versioned bytes APIs remain available on the container types ({class}`~ommx.Instance`, {class}`~ommx.ParametricInstance`, {class}`~ommx.Solution`, {class}`~ommx.SampleSet`) and on the cross-evaluate DTOs ({class}`~ommx.State`, {class}`~ommx.Samples`, {class}`~ommx.Parameters`) — use `to_v1_bytes` / `from_v1_bytes` or `to_v2_bytes` / `from_v2_bytes` where available when you need to persist or exchange data on disk or over the wire.

### 🆕 Write-through label/context wrappers: `AttachedConstraint` / `AttachedDecisionVariable` ([#849](https://github.com/Jij-Inc/ommx/pull/849), [#850](https://github.com/Jij-Inc/ommx/pull/850), [#852](https://github.com/Jij-Inc/ommx/pull/852))

`Instance.add_constraint` / `instance.constraints[id]` and the matching accessors on `ParametricInstance` now return write-through handles bound to the parent host instead of snapshot copies. Reads pull live data from the host and label/context setters write straight to its SoA stores, so two handles pointing at the same id observe the same state.

```python
c = instance.add_constraint(x + y == 0)         # AttachedConstraint
c.set_name("budget")                             # writes through to instance
assert instance.constraints[c.constraint_id].name == "budget"
```

Five write-through types ship: {class}`~ommx.AttachedConstraint`, {class}`~ommx.AttachedIndicatorConstraint`, {class}`~ommx.AttachedOneHotConstraint`, {class}`~ommx.AttachedSos1Constraint`, and {class}`~ommx.AttachedDecisionVariable`. {class}`~ommx.Constraint` and {class}`~ommx.DecisionVariable` are unchanged in shape — they remain the snapshot wrappers used for modeling input (operator overloading, `Instance.from_components`). Each `AttachedX` exposes `.detach()` to obtain an equivalent snapshot when you need to break the back-reference to the host.

As part of the same change, `instance.decision_variables` now returns `list[AttachedDecisionVariable]` (previously `list[DecisionVariable]` snapshots), aligning with `instance.constraints` and the special-constraint accessors.

### 🆕 OpenTelemetry-based tracing and profiling ([#816](https://github.com/Jij-Inc/ommx/pull/816), [#823](https://github.com/Jij-Inc/ommx/pull/823), [#826](https://github.com/Jij-Inc/ommx/pull/826), [#828](https://github.com/Jij-Inc/ommx/pull/828), [#829](https://github.com/Jij-Inc/ommx/pull/829))

The legacy `log` + `pyo3-log` → Python `logging` bridge is replaced by a `tracing` + `pyo3-tracing-opentelemetry` pipeline, so the Rust core's spans can now be consumed through the Python OTel SDK.

Two entry points ship under `ommx.tracing`:

- **`%%ommx_trace`** — a Jupyter cell magic that renders a per-cell span tree and a Chrome Trace JSON download link
- **`capture_trace` / `@traced`** — a context manager and decorator for the same workflow from regular Python scripts, tests, and CI

See [Tracing and Profiling](../user_guide/tracing.ipynb) for the full walkthrough, configuring your own `TracerProvider`, and troubleshooting.

### 🆕 Tracing spans in solver/sampler adapters ([#833](https://github.com/Jij-Inc/ommx/pull/833))

Every OMMX adapter now emits three OpenTelemetry spans per solve/sample call, so the OTel tracing pipeline above can attribute wall-clock time to the three phases an adapter actually spends time in:

- **`convert`** — OMMX `Instance` → solver-native problem translation
- **`solve`** / **`sample`** — the call into the underlying solver / sampler itself
- **`decode`** — decoding the solver's response back to `Solution` / `SampleSet` (Rust-side `evaluate` spans nest underneath)

Each adapter uses its own tracer name, so runs from different solvers are easy to distinguish in the tree view:

| Adapter | Tracer | Spans |
|---|---|---|
| `ommx-pyscipopt-adapter` | `ommx.adapter.pyscipopt` | `convert` / `solve` / `decode` |
| `ommx-highs-adapter` | `ommx.adapter.highs` | `convert` / `solve` / `decode` |
| `ommx-python-mip-adapter` | `ommx.adapter.python_mip` | `convert` / `solve` / `decode` |
| `ommx-openjij-adapter` | `ommx.adapter.openjij` | `convert` / `sample` / `decode` |

```python
from ommx.tracing import capture_trace, render_text_tree
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

with capture_trace() as trace:
    solution = OMMXPySCIPOptAdapter.solve(instance)

print(render_text_tree(trace))  # shows convert / solve / decode with durations
```

Spans are emitted through the standard OpenTelemetry API, so they are a no-op when no `TracerProvider` is installed — there is no runtime cost for users who do not opt in.

### 🆕 `Function.evaluate_bound` is now available from Python ([#831](https://github.com/Jij-Inc/ommx/pull/831))

{meth}`Function.evaluate_bound <ommx.Function.evaluate_bound>` is now exposed on {class}`~ommx.Function`. Given per-variable bounds, it returns a {class}`~ommx.Bound` that contains the range of the function value — useful when deriving feasibility bounds or doing simple presolve on the Python side.

```python
from ommx import Function, Linear, Bound

f = Function(Linear(terms={1: 2}, constant=3))  # 2*x1 + 3
b = f.evaluate_bound({1: Bound(0.0, 2.0)})
# b.lower == 3.0, b.upper == 7.0
```

The bound is computed monomial-wise and summed, so it is a sound over-approximation of the true range but is **not guaranteed to be tight** when multiple terms share variables (the classic dependency problem in interval arithmetic). Variable IDs missing from `bounds` are treated as unbounded.

## 3.0.0 Alpha 2

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a2-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a2)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### ⚠ Removal of the `Constraint.id` field ([#806](https://github.com/Jij-Inc/ommx/pull/806))

The `id` field (along with the `.id` getter, `set_id()`, and `id=` constructor argument) is removed from {class}`~ommx.Constraint` and its variants ({class}`~ommx.IndicatorConstraint` / {class}`~ommx.OneHotConstraint` / {class}`~ommx.Sos1Constraint` / {class}`~ommx.EvaluatedConstraint` / {class}`~ommx.SampledConstraint` / {class}`~ommx.RemovedConstraint`). A constraint's ID now exists only as the key of the `dict[int, Constraint]` passed to {meth}`Instance.from_components <ommx.Instance.from_components>`.

```python
# Before (2.5.1)
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO, id=5)
Instance.from_components(..., constraints=[c], ...)

# After (3.0.0a2)
c = Constraint(function=x + y, equality=Constraint.EQUAL_TO_ZERO)
Instance.from_components(..., constraints={5: c}, ...)
```

Global ID counters (`next_constraint_id` and friends) and per-constraint `to_bytes` / `from_bytes` are also removed. For full details and migration steps, see the [Python SDK v2 to v3 Migration Guide](../migration/python_sdk_v2_to_v3.md).

### 🆕 First-class special constraint types ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#790](https://github.com/Jij-Inc/ommx/pull/790), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#796](https://github.com/Jij-Inc/ommx/pull/796), [#798](https://github.com/Jij-Inc/ommx/pull/798))

In addition to regular constraints, the following three special constraint types are now first-class citizens — they can be passed to `Instance.from_components` via `indicator_constraints=` / `one_hot_constraints=` / `sos1_constraints=`, and read back through {meth}`~ommx.Solution.constraints_df` / {meth}`~ommx.SampleSet.constraints_df` with `kind=` selecting the family.

- {class}`~ommx.IndicatorConstraint` — conditional constraint on a binary variable (new)
- {class}`~ommx.OneHotConstraint` — replaces the previous `ConstraintHints.OneHot` metadata
- {class}`~ommx.Sos1Constraint` — replaces the previous `ConstraintHints.Sos1` metadata

For concrete usage, evaluation-result access, and the Indicator relax / restore workflow, see [Special Constraints](../user_guide/special_constraints.md).

Accordingly, the legacy `ConstraintHints` / `OneHot` / `Sos1` classes, the `Instance.constraint_hints` property, and the PySCIPOpt Adapter's `use_sos1` flag are removed.

### 🆕 Legacy `ADDITIONAL_CAPABILITIES` lowering API (superseded) ([#790](https://github.com/Jij-Inc/ommx/pull/790), [#805](https://github.com/Jij-Inc/ommx/pull/805), [#810](https://github.com/Jij-Inc/ommx/pull/810), [#811](https://github.com/Jij-Inc/ommx/pull/811), [#814](https://github.com/Jij-Inc/ommx/pull/814))

This subsection records the API introduced in 3.0.0 Alpha 2. That API was
superseded before the current release: `AdditionalCapability`,
`ADDITIONAL_CAPABILITIES`, and constructor-driven automatic lowering are not
the current Adapter Capability model.

At Alpha 2, adapters selected special-constraint families through an
`ADDITIONAL_CAPABILITIES` class attribute. Calling `super().__init__(instance)`
converted every unselected special constraint to regular constraints (Big-M
for Indicator / SOS1, linear equality for OneHot) before the instance reached
the solver. Existing adapters at that point had to add that constructor call.

Current adapters instead declare complete native translator profiles through
`CAPABILITIES` and reject incompatible inputs without mutation. Explicit
special-constraint lowering uses {class}`~ommx.SpecialConstraintKind` and
{meth}`~ommx.Instance.lower_special_constraints`, followed by another native
compatibility check. These transformation selectors are also unrelated to
`ommx.v2.Feature`, which only governs serialization forward compatibility. See
[Adapter Capability Model and Explicit Preparation](../user_guide/capability_model.md).

### 🔄 numpy scalar support ([#794](https://github.com/Jij-Inc/ommx/pull/794))

The {class}`~ommx.Function` constructor now accepts `numpy.integer` and `numpy.floating` values. In v2.5.1, `Function(numpy.int64(3))` raised `TypeError`.

## 3.0.0 Alpha 1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a1-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a1)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### Complete Rust re-export of `ommx` and `ommx.artifact` types ([#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775), [#782](https://github.com/Jij-Inc/ommx/pull/782))

Python SDK 3.0.0 is fully based on Rust/PyO3.
In 2.0.0, the core implementation was rewritten in Rust while Python wrapper classes remained for compatibility. In 3.0.0, those Python wrappers are removed entirely — all types in `ommx` and `ommx.artifact` are now direct re-exports from Rust, and the `protobuf` Python runtime dependency is eliminated. The `.raw` attribute that previously provided access to the underlying PyO3 implementation has also been removed.

### Migration to Sphinx and ReadTheDocs hosting ([#780](https://github.com/Jij-Inc/ommx/pull/780), [#785](https://github.com/Jij-Inc/ommx/pull/785))

In v2, the Sphinx-based API Reference and Jupyter Book-based documentation were each hosted on [GitHub Pages](https://jij-inc.github.io/ommx/en/introduction.html). In v3, documentation has been fully migrated to Sphinx and is now hosted on [ReadTheDocs](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/). GitHub Pages will continue to host the documentation as of v2.5.1, but all future updates will be on ReadTheDocs only.
