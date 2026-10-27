---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# What transformations preserve

Users transform problem representations for different tasks: passing them to a solver, reducing variables, or studying particular conditions. **Successful transformation, reading candidates in original variables, and obtaining an optimum of the original problem are different claims.** Check whether an operation's guarantees are sufficient for your purpose.

This page assumes that Instance represents a problem and State an assignment of variable values. It covers operation choices and result interpretation, leaving individual formulas and applicability conditions to the API Reference. To try the procedure first, use [Transform an Instance and inspect results](../how_to/transform_instance_without_solver.md).

## First identify what the operation changes

“Transformation” covers different changes and candidate correspondences.

| Example operation | What changes | What the user checks |
|---|---|---|
| Replace OneHot with an ordinary equality | The representation of the same condition | Variable domains and lowering conditions |
| Encode an Integer with Binary variables | Variables selected and their mapping to the original variable | Coverage of the allowed integer range and required auxiliary conditions |
| Fix a variable | The candidate search space | Whether the guarantee covers the original problem or a conditioned subproblem |
| Relax a constraint | The allowed candidate set | Whether candidates from the relaxation satisfy original conditions |
| Move a constraint into a penalty | The objective being compared and currently imposed constraints | Which candidates receive penalties, and whether the weight suits the task |

Do not extend an explanation valid for one operation to every operation. Even an encoding that reconstructs original values still requires a separate check of original constraints.

## Seven questions for reading guarantees

Separate these questions when reading an operation's API contract.

| Question | What to check |
|---|---|
| How do candidates correspond? | Same variables, auxiliary variables, or reconstruction through fixed values and dependencies |
| In which direction does feasibility transfer? | Can transformed feasible candidates be read back, and can every original feasible candidate be represented? |
| How do objective values correspond? | Same values/order, changed sign/constant, or a different penalty objective |
| Over what scope does optimality transfer? | Is the correspondence sufficient for optima, or only within a conditioned subproblem? |
| What numerical conditions apply? | Domains, bounds, tolerances, normalization, and operation-specific preconditions |
| Against which Instance is a candidate evaluated? | The corresponding original problem or a transformed Instance retaining its evaluation information |
| Is the guarantee sufficient for my task? | Is only the optimum value needed, or must every original feasible candidate remain representable? |

Preserving the optimal value, for example, does not necessarily preserve every feasible candidate. Enumerating candidates or studying a sampling distribution may require more than an optimal-value guarantee.

## Separate a conditioned subproblem from the whole original problem

Minimizing a Binary variable $x$ has optimum 0. The subproblem conditioned on $x=1$ has optimum 1.

```{code-cell} ipython3
from ommx import Instance

source = Instance.minimize()
x = source.new_binary("x")
source.objective = x

original_candidate = source.evaluate({x.id: 0})
conditioned = source.partial_evaluate({x.id: 1})
conditioned_candidate = conditioned.evaluate({})

assert original_candidate.feasible and original_candidate.objective == 0.0
assert conditioned_candidate.feasible and conditioned_candidate.objective == 1.0
assert conditioned_candidate.state.entries[x.id] == 1.0
```

Reconstructing the fixed value and obtaining a feasible candidate do not imply global optimality for the original problem. The optima above follow from comparing the two Binary values; `evaluate()` did not establish optimality.

Conversely, fixing $x=1$ when the original constraints already imply it can simplify without further restricting the candidate set. Rather than classifying all fixing as nonequivalent, ask **why that value can be fixed and over which set the guarantee holds**. For a general model, successful {meth}`~ommx.Instance.partial_evaluate` does not establish feasibility of remaining constraints. Evaluate the resulting candidate explicitly.

## Squaring an inequality body can reorder feasible candidates

Consider this penalty example:

$$
\min x \quad\text{subject to}\quad x-1\leq0,\quad x\in\{0,1\}.
$$

Both 0 and 1 are feasible, and the original objective prefers 0. Squaring the entire inequality body with weight 2 gives $x+2(x-1)^2$.

| $x$ | Original objective $x$ | Squared body $(x-1)^2$ | Objective with penalty |
|---|---|---|---|
| 0 | 0 | 1 | 2 |
| 1 | 1 | 0 | 1 |

The feasible value 0 receives a penalty, reversing the order. In general, increasing the weight does not ensure recovery of the original optimum. This differs from squaring only the violation, $\max(0,x-1)^2$, which is zero for both candidates.

OMMX's fixed-weight penalty operation also squares ordinary-constraint bodies. Decide what to penalize, for example by first converting an inequality to an equality with suitable slack variables. See {meth}`~ommx.FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight` and {meth}`~ommx.Instance.convert_inequality_to_equality_with_integer_slack` for exact conditions and processing. An Adapter's name alone does not determine suitable weights or slack conditions.

## Using several operations

Check conditions passed to subsequent operations and the correspondence needed to interpret the final candidate. For example, in “encode integers → penalize constraints → convert the objective to minimization,” original integer reconstruction, post-penalty feasibility, and the original objective's sign each need their own explanation.

[InstanceClass](./instance_class.md) membership checks required representation conditions. [PreparationPolicy](./preparation_policy.md) selects allowed operations. Even when `target.contains(instance)` is true after Preparation, that Boolean alone establishes neither mathematical correspondence with the source nor execution-result quality or optimality.

Also distinguish ideal mathematical correspondence from API contracts involving floating-point tolerances. Rounding a near-Binary value and testing an ordinary constraint each follow their public contracts. Matching values before and after transformation for a few candidates verifies that example; it does not establish a guarantee for arbitrary problems.

## Which problem defines the result?

Values returned by another system alone do not determine original feasibility or objective values. Evaluate them using the corresponding Instance's domains, constraints, and objective. A transformed Instance may retain fixed values, dependencies, removed constraints, and an output objective for inspecting reconstructed values and results. Retained expressions are not necessarily immutable copies through every later transformation.

Keep the original Instance itself when you need to evaluate or reuse it later. Comparing evaluations before and after transformation only requires saved prior results; `prepare()` examples do not copy an Instance solely for that comparison.

See [Variable roles, reconstruction, and defaulting](./variable_roles.md) for value sources, and [Transformed problems and removed constraints](./removed_constraints.md) for retained conditions, reported objectives, and the scope of optimality mapping. Mapping optimality of the active problem to the output objective is not the same as guaranteeing optimality for the whole original problem including removed constraints.
