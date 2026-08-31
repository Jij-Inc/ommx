# Instance Preparation and `PreparationPolicy`

Instance Preparation applies existing transformation operations to a chosen {class}`~ommx.Instance`, moving it toward membership in a target {class}`~ommx.InstanceClass`. The easy Adapter APIs perform this workflow on an isolated copy. Applications perform it explicitly when they need to inspect or customize the transformation choices.

```{important}
Use `solve()` or `sample()` for the recommended workflow. When the application changes the Policy, it also owns the Instance passed to {meth}`~ommx.Instance.prepare` and then uses the corresponding `*_without_preparation()` API.
```

See [Adapter Exact Inputs (`INPUT_CLASS`)](./adapter_input_class.md) for the exact set represented by `INPUT_CLASS`. This page explains the two ways to prepare an Instance toward that set.

## Recommended and Customized Execution

{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy` returns a {class}`~ommx.PreparationPolicy` that is generally useful for reaching the Adapter's exact input.

The method has the following contract.

- It returns a fresh Policy that the caller may edit on every call.
- It does not inspect or mutate a particular Instance.
- Calling the recommendation method alone does not execute Preparation.
- The Policy does not by itself guarantee target membership; `prepare()` establishes membership or raises an error.

For the usual workflow, the easy API copies the source, obtains the recommendation, prepares the copy for `INPUT_CLASS`, and calls the strict API. The source remains unchanged.

```python
result = Adapter.solve(source)
```

When an application needs different choices, it performs those same steps explicitly:

```python
import copy

input_class = Adapter.INPUT_CLASS
policy = Adapter.recommended_preparation_policy()

# Edit the policy here when the application needs different choices.
prepared = copy.copy(source)
prepared.prepare(input_class, policy)

# Successful Preparation guarantees the complete applicability condition.
assert input_class.contains(prepared)
result = Adapter.solve_without_preparation(prepared)
```

`prepare()` mutates the supplied Instance in place. If the source model will also be used with another Adapter, or if you need to compare the model before and after Preparation, decide which value to preserve and copy it explicitly. See [Prepare an Instance for an Adapter](../tutorial/prepare_instance_for_adapter.md) for a runnable example.

## Phases Selected by `PreparationPolicy`

Every field of `PreparationPolicy` is optional, and an unspecified phase does not run. All phases are disabled by default. A field does not implement a transformation itself; it selects which existing operation owned by `Instance` will be called. The selected owner operation continues to define its input conditions and errors.

{meth}`~ommx.Instance.prepare` applies each selected phase at most once, in this fixed order.

| Order | Policy field | Role |
|---|---|---|
| 1 | `special_constraints` | Lower selected active special constraints to regular constraints |
| 2 | `objective` | Convert the active objective to the requested optimization sense |
| 3 | `integer_slack` | Introduce integer slack variables for active regular inequalities |
| 4 | `fixed_penalty` | Move active regular constraints into the objective with a fixed-weight penalty |
| 5 | `integer_encoding` | Encode used integer variables with binary variables |
| 6 | `binary_power_reduction` | Reduce powers of active Binary decision variables |

See the API Reference for {class}`~ommx.SpecialConstraintPreparation`, {class}`~ommx.ObjectivePreparation`, {class}`~ommx.IntegerSlackPreparation`, {class}`~ommx.FixedPenaltyPreparation`, {class}`~ommx.IntegerEncodingPreparation`, and {class}`~ommx.BinaryPowerPreparation` for the type and complete set of factory arguments accepted by each field. The formulas and validity conditions for lowering special constraints belong to the API Reference for each conversion method. See [Special Constraint Types](./special_constraints.md) for the mathematical meaning of the constraints themselves.

Some choices, such as the magnitude of a fixed penalty, depend on the scale of the problem and the properties required of its solutions; no universally safe value exists. The application must choose such values explicitly instead of delegating them to an Adapter recommendation.

## Target-Directed Execution

Preparation is not a batch recipe that necessarily runs every Policy phase. Before execution and after each selected phase completes, `prepare()` checks the whole Instance against the target. It stops as soon as the Instance is a member, so selected phases after that point do not run. If the Instance is already a member, it remains unchanged.

Successful Preparation has one postcondition:

```python
assert input_class.contains(instance)
```

This membership is the complete Adapter applicability condition. A strict Adapter API checks it again and may still report a separate converter or Backend failure later.

When converting Backend output back to a {class}`~ommx.Solution` or {class}`~ommx.SampleSet`, the same prepared Instance passed to the Adapter is also the owner of evaluation. Its decision-variable dependencies and removed constraints let OMMX recover pre-encoding variable values and evaluate pre-transformation constraints. See [Removed Constraints and Feasibility](./removed_constraints.md) for those evaluation rules.

## Unreachable Targets and Partial Mutation

If all configured phases run without reaching the target, {class}`~ommx.PreparationTargetNotReachedError` is raised. Its `report` attribute contains an {class}`~ommx.InstanceClassMembershipReport` for the final state. The caller can inspect the report and edit the Policy, add another phase, or select a different target.

There is no global transaction around an entire Preparation. If a later phase or owner operation fails, mutations completed by earlier phases remain in the Instance. Each owner operation defines how much it validates before mutation in its own API contract. With the easy `solve()` and `sample()` APIs, those partial changes remain confined to the internal copy. When calling `prepare()` explicitly, make a copy first if the source must remain in its pre-Preparation state after a failure.

These failure semantics are an advanced part of Preparation. Start with the happy path in the Tutorial, then return to this section and the owner operation's API Reference when you need to handle failures.
