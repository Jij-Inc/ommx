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

# Instance: the basic optimization-problem model

An {class}`~ommx.Instance` combines decision-variable domains (allowed kinds and ranges of values), an objective, constraints, and the optimization sense. Rather than being the input format of one solver, it defines in OMMX **which assignments are feasible and what objective values they have**. Candidates can therefore be evaluated without solving the problem.

For example, create a problem in which two items cannot be selected together.

$$
\begin{aligned}
\max \quad & x + y \\
\text{subject to} \quad & xy = 0 \\
& x, y \in \{0, 1\}.
\end{aligned}
$$

```{code-cell} ipython3
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
exclusive = instance.add_constraint(x * y == 0, "exclusive")

solution = instance.evaluate({x.id: 1, y.id: 0})
assert solution.feasible
assert solution.objective == 1
```

`new_binary` and `add_constraint` assign IDs automatically. Access them through attributes such as `x.id`. [Evaluating a State to obtain a Solution](./solution.md) distinguishes a {class}`~ommx.State` assignment from its {class}`~ommx.Solution` evaluation result. Evaluation checks a candidate; it does not search for an optimal one.

## Information defining a problem

| Information | Meaning | Reference |
|---|---|---|
| Decision variables | Allowed kinds and ranges of values | {attr}`~ommx.Instance.decision_variables` |
| Objective | The quantity to compare | {attr}`~ommx.Instance.objective` |
| Constraints | Conditions to satisfy | {attr}`~ommx.Instance.constraints` |
| Optimization sense | Whether larger or smaller objective values are preferred | {attr}`~ommx.Instance.sense` |

Create a minimization problem with {meth}`~ommx.Instance.minimize`. Objectives and ordinary-constraint expressions are represented by {class}`~ommx.Function`. See [Function](./function.md) for expression construction, and [Special constraints](./special_constraints.md) for Indicator, OneHot, and SOS1 semantics.

Variables and constraints are identified by IDs, not names. Different IDs identify different objects even if names match. Use {meth}`~ommx.Instance.from_components` to assemble existing components with explicit IDs. An Instance validates that variables referenced by its expressions and special constraints are defined within it.

(decision-variable-domains)=
## Variable domains and metadata

A decision variable's domain is specified by `kind` and its `lower` and `upper` bounds. Create variables on an Instance as follows.

```{code-cell} ipython3
typed = Instance.minimize()
flag = typed.new_binary("flag")
count = typed.new_integer("count", lower=0, upper=10)
amount = typed.new_continuous("amount", lower=0)
batch = typed.new_semi_integer("batch", lower=2, upper=10)
rate = typed.new_semi_continuous("rate", lower=0.5, upper=4)
```

Binary takes 0 or 1, Integer takes integers, and Continuous takes real values. SemiInteger and SemiContinuous also allow zero in addition to integers or real values within the specified range. For example, `batch` allows zero or integers from 2 to 10; `rate` allows zero or real values from 0.5 to 4. See each `new_*` method's API Reference for bound normalization and tolerances.

`name`, `subscripts`, `parameters`, and `description` are metadata for analysis and visualization. They do not change variable domains or constraint satisfaction conditions. For example, store a variable $x_i$ with `name="x"` and `subscripts=[i]`.

```{code-cell} ipython3
instance.decision_variables_df()
```

{meth}`~ommx.Instance.decision_variables_df` returns definitions and labels as a pandas DataFrame. Retrieve a single variable by ID with {meth}`~ommx.Instance.get_decision_variable_by_id`.

## Retained variables and currently used variables

In an ordinary formulation, you might expect every declared variable to be passed to the solver. Fixing a variable or expressing it through other variables can make it unnecessary for the solver to choose its value. OMMX retains its definition so that results can still be read in terms of original variables.

{attr}`~ommx.Instance.decision_variables` returns all retained variables; {attr}`~ommx.Instance.used_decision_variables` returns **variables appearing in the current objective and currently imposed constraints**.

[Variable roles, reconstruction, and defaulting](./variable_roles.md) explains why these collections differ, and how fixed values, dependencies, and defaults are used during evaluation. For a first solve or candidate evaluation, begin by supplying values for variables used in the problem and reading the result.

## Inspect ordinary constraints

Ordinary constraints are stored as $f(x)=0$ or $f(x)\leq0$. For example, $f(x)\geq0$ is rewritten as $-f(x)\leq0$. {attr}`~ommx.Constraint.equality` distinguishes equalities from inequalities.

```{code-cell} ipython3
instance.constraints_df()
```

{attr}`~ommx.Instance.constraints` is a dictionary keyed by constraint ID. Constraints support metadata similar to variables. Their ID space is independent of variable IDs, and each special-constraint family has its own constraint-ID space.

(simultaneous-bound-tightening)=
## Transforming a model

An Instance can be transformed after construction. However, **being able to evaluate the result is different from equivalence with the original optimization problem**.

For operations such as fixing variables, substituting expressions, or rewriting constraints, check what is preserved. [What transformations preserve](./transformation_guarantees.md) separates feasibility, objective values, and optimality. To try a complete procedure, see [Transform an Instance and inspect results](../how_to/transform_instance_without_solver.md).

{meth}`~ommx.Instance.tighten_bounds_simultaneously_once` tightens variable bounds from ordinary constraints. See its API Reference for supported expressions, tolerances, numerical restrictions, and constraint selection.

### Information retained for interpreting results

An Instance can retain information beyond its current objective and constraints. For example, a {class}`~ommx.NamedFunction` is a named expression whose value can be inspected independently of objectives and constraints.

Constraints excluded by a transformation but retained for checking results are called removed. A reporting objective separate from the solver-facing one is called the output objective. Appearing only in named functions, removed constraints, or the output objective does not itself make a variable used.

If such values are omitted, evaluation uses reconstructed or defaulted values. Supply explicit values to inspect a particular candidate. [Variable roles](./variable_roles.md) explains where values come from; [Transformed problems and removed constraints](./removed_constraints.md) explains retained constraints and objective values.
