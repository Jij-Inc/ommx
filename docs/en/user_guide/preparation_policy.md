# Instance Preparation and PreparationPolicy

Instance Preparation applies existing transformations to an {class}`~ommx.Instance` toward a representation belonging to a target {class}`~ommx.InstanceClass`. **Separate the target class, the Policy selecting allowed operations, and `Instance.prepare()` performing the transformation.**

| Component | Role |
|---|---|
| Target InstanceClass | Required representation conditions |
| PreparationPolicy | Allowed transformation operations and settings |
| Instance.prepare | Apply transformations to that Instance and check target membership |

See [InstanceClass and Adapter input conditions](./instance_class.md) for classes and [Transform an Instance](../how_to/transform_instance_without_solver.md) for a complete procedure. Later sections apply this common mechanism to Adapters.

## Creating a Policy is not performing a transformation

A {class}`~ommx.PreparationPolicy` is a value holding operation choices. Creating or editing one does not change an Instance.

For example, this Policy replaces a OneHot “choose exactly one” condition with an ordinary equality. Rewriting special constraints as ordinary constraints is called lowering. Here, assume that `instance` and the target `target` are already available.

```python
from ommx import (
    PreparationPolicy, SpecialConstraintKind, SpecialConstraintPreparation,
)

policy = PreparationPolicy(
    special_constraints=SpecialConstraintPreparation.lower_special_constraints(
        kinds={SpecialConstraintKind.OneHot}
    )
)

instance.prepare(target, policy)
assert target.contains(instance)
```

{meth}`~ommx.Instance.prepare` modifies its Instance in place. Make a prior copy only when the original Instance itself must remain available later. Comparing candidate evaluations requires only the evaluation results saved beforehand. [The transformation How-to](../how_to/transform_instance_without_solver.md) also supplies construction of the Instance and target.

## Stop when the target is reached

Preparation does not necessarily execute every Policy operation to the end. It checks whole-Instance membership before execution and after each selected phase, stopping when the target is reached. An Instance already belonging to the target remains unchanged.

Successful Preparation establishes membership of the specified target.

```python
assert target.contains(instance)
```

That condition alone does not guarantee mathematical equivalence or optimality relative to the original problem. Check source conditions, selected operations, and what each preserves.

[What transformations preserve](./transformation_guarantees.md) explains what to check for candidates, feasibility, objective values, and optimality.

Interpret transformed candidates using the variables, constraints, and objective information retained by that Instance. See {ref}`Variable roles <instance-variable-roles>` and [Transformed problems and removed constraints](./removed_constraints.md).

## Phases selected by PreparationPolicy

Each `PreparationPolicy` field is optional; an unspecified phase is not executed. All phases are disabled by default. Fields select which Instance transformation operations to invoke. See the corresponding methods' API Reference for inputs and errors.

{meth}`~ommx.Instance.prepare` applies each selected phase at most once in this fixed order.

| Order | Policy field | Role |
|---|---|---|
| 1 | `special_constraints` | Lower selected active special constraints to ordinary constraints |
| 2 | `objective` | Convert the active objective to the specified sense |
| 3 | `integer_slack` | Introduce Integer slack for active ordinary inequalities |
| 4 | `integer_encoding` | Encode used Integer variables with Binary variables |
| 5 | `fixed_penalty` | Move active ordinary constraints into fixed-weight objective penalties |
| 6 | `binary_power_reduction` | Reduce powers of active Binary variables |

See {class}`~ommx.SpecialConstraintPreparation`, {class}`~ommx.ObjectivePreparation`, {class}`~ommx.IntegerSlackPreparation`, {class}`~ommx.FixedPenaltyPreparation`, {class}`~ommx.IntegerEncodingPreparation`, and {class}`~ommx.BinaryPowerPreparation` for field types and factory arguments. Individual methods' API Reference owns special-constraint lowering formulas and conditions. [Special constraints](./special_constraints.md) explains the constraints themselves.

Some choices, such as fixed penalty magnitudes, depend on problem scale and the candidates sought. The application decides which transformations to allow and which values to use.

## Separate QUBO Preparation from coefficient formatting

{meth}`~ommx.PreparationPolicy.for_qubo` provides a general QUBO-oriented Policy. It only creates a Policy; it does not change an Instance. Prepare explicitly toward `InstanceClass.qubo()`, then format coefficients if needed.

```python
from ommx import InstanceClass, PreparationPolicy

policy = PreparationPolicy.for_qubo(uniform_penalty_weight=2.0)
instance.prepare(InstanceClass.qubo(), policy)
coefficients, offset = instance.as_qubo_format()
```

Here, 2.0 is an example user-selected weight, not a sufficient value for every problem. `for_qubo()` also has defaults for omitted arguments. Factory defaults do not guarantee desired candidates for a particular problem. See [Transformation guarantees](./transformation_guarantees.md) especially for penalties squaring whole inequality bodies.

{meth}`~ommx.Instance.to_qubo` combines QUBO-oriented Preparation with active-objective coefficient formatting and changes the Instance in place. {meth}`~ommx.Instance.as_qubo_format` does not perform Preparation. Both return coefficients of the active objective, whose value need not always match the reported objective returned by `instance.evaluate()`.

The prepared object remains an OMMX Instance. It can retain information for variable reconstruction, constraints, and output objectives, but passing only coefficients and an offset does not automatically carry that information. Keep working with the Instance when evaluating or saving problems and candidates in OMMX.

The generic `for_qubo()` Policy and a concrete Adapter's recommendation are separate choices. OpenJij SA, for example, accepts higher-degree polynomials directly and leaves fixed penalty weights to the caller in its recommendation. [Sample a small problem with OpenJij](../how_to/sample_with_openjij_adapter) starts from that Adapter's target and recommendation.

## Application to Adapters: recommendations and automatic Preparation

An Adapter declares the Instances it accepts directly as `INPUT_CLASS`. Using that InstanceClass as a target applies Preparation to the Adapter.

For ordinary solving or sampling, start with `solve()` or `sample()`. Standard easy APIs copy the input, prepare that copy using the Adapter's recommendation, and invoke the strict method.

```python
# 呼び出し元のinstanceは変更されない。内部のcopyだけがPreparationされる。
result = Adapter.solve(instance)
```

{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy` returns a Policy generally useful for reaching the Adapter's exact input. The call:

- Returns a fresh Policy that the caller can edit each time
- Does not inspect or change a particular Instance or perform Preparation
- Does not guarantee reaching target membership

`INPUT_CLASS` and the recommendation are independent values. If an application-specific choice, such as a fixed penalty magnitude, cannot safely be recommended by the Adapter, edit the Policy and use the explicit path.

```python
input_class = Adapter.INPUT_CLASS
policy = Adapter.recommended_preparation_policy()

# 必要なら、application固有の判断でpolicyを編集する。
instance.prepare(input_class, policy)

assert input_class.contains(instance)
result = Adapter.solve_without_preparation(instance)
```

`*_without_preparation()` and Adapter constructors do not prepare. This path applies your edited Policy before passing the Instance. See [Choose constraint families for Preparation](../how_to/customize_preparation.md) for a complete example.

Adapter applicability is `INPUT_CLASS` membership. A converter or backend may subsequently return other errors during solver-input construction or execution; these are not additional applicability conditions.

When decoding solver output to a {class}`~ommx.Solution` or {class}`~ommx.SampleSet`, evaluation also uses the same transformed Instance passed to the Adapter. Its dependencies and removed constraints allow evaluation of pre-encoding variable values and pre-transformation conditions. See [Variable roles, reconstruction, and defaulting](./variable_roles.md) and [Removed constraints and feasibility](./removed_constraints.md).

## Detail: unreachable targets and partial changes

If all configured phases finish without reaching the target, {class}`~ommx.PreparationTargetNotReachedError` is raised. Its `report` is an {class}`~ommx.InstanceClassMembershipReport` for the final state. Inspect it to add or change Policy choices or select a different target.

Failure of Preparation does not automatically restore the original state. If a later phase fails, earlier completed changes remain in the Instance. With ordinary `solve()` and `sample()`, those changes remain only in the internal copy. Before explicit `prepare()`, make a copy only if the original Instance itself must remain available after failure. See each operation's API Reference for its own failure-mutation guarantees.

For first Adapter use, start with the [Tutorial](../tutorial/prepare_instance_for_adapter.md), returning to this section and individual API contracts when you need failure handling.
