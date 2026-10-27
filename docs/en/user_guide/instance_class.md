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

# InstanceClass and Adapter input conditions

An {class}`~ommx.InstanceClass` is a set of {class}`~ommx.Instance` objects satisfying specified representation conditions. For example, it can describe “maximization problems using Binary variables and linear equalities only.” **It checks whether the current Instance satisfies those conditions; it does not transform it.**

InstanceClass is used for representation checks and transformation targets. An Adapter's `INPUT_CLASS` is one application. This page explains class conditions and membership, then applies them to Adapters.

## The set represented by an InstanceClass

An InstanceClass is a finite union of {class}`~ommx.InstanceClassClause` objects: an Instance must satisfy at least one clause. Within each clause, all of the following conditions must hold together.

- Kinds of used decision variables
- Objective polynomial representation and degree
- Ordinary-constraint relations (equalities/inequalities), polynomial representation, and degree
- Indicator-body relations, polynomial representation, and degree
- Whether active OneHot and SOS1 constraints are allowed
- Optimization sense (maximize/minimize)

Declaring separate clauses for “continuous quadratic programs” and “linear programs allowing integers,” for example, does not automatically allow mixed-integer quadratic programs by combining conditions across clauses.

## Check membership

Membership means that an Instance satisfies a class's conditions and belongs to its set. {meth}`~ommx.InstanceClass.contains` returns a `bool`. For reasons for non-membership, {meth}`~ommx.InstanceClass.check_membership` returns an {class}`~ommx.InstanceClassMembershipReport` containing clause-level results. Neither has side effects.

Use the environment from {ref}`SDK installation <install-python-sdk>`. The following class permits Binary variables, a linear objective, and ordinary linear equalities. It does not permit special constraints such as OneHot.

```{code-cell} ipython3
from ommx import (
    Equality, InstanceClass, InstanceClassClause, Kind,
    PolynomialRequirement, Sense,
)

linear = PolynomialRequirement.at_most(1)
target = InstanceClass([
    InstanceClassClause(
        label="binary-linear-equality",
        allowed_variable_kinds={Kind.Binary},
        objective_polynomial_requirement=linear,
        regular_constraint_polynomial_requirements={Equality.EqualToZero: linear},
        allowed_senses={Sense.Maximize},
    )
])
```

OneHot means “choose exactly one.” Although it can be expressed mathematically as a linear equality, retaining it as a dedicated special constraint does not satisfy a class allowing only ordinary equalities. A membership check does not lower OneHot.

```{code-cell} ipython3
from ommx import Instance, OneHotConstraint

instance = Instance.maximize()
xs = [instance.new_binary("x", subscripts=[i]) for i in range(2)]
instance.objective = 3 * xs[0] + 2 * xs[1]
instance.add_one_hot_constraint(OneHotConstraint(variables=xs))

assert not target.contains(instance)

report = target.check_membership(instance)
assert not report.is_member
assert instance.one_hot_constraints
print(report)
```

See {class}`~ommx.InstanceClassMismatch` and {class}`~ommx.InstanceClassClauseReport` for mismatch types and attributes returned by `check_membership()`.

[Transform an Instance](../how_to/transform_instance_without_solver.md) demonstrates converting this Instance to ordinary equalities. [Instance Preparation and PreparationPolicy](./preparation_policy.md) explains the relationship between the target class and a Policy selecting permitted transformations.

## Mathematical conditions and representation are separate

Membership also checks `Function` representation. For a linear expression `g` retained as a `Function`, `g ** 2` is not automatically expanded. Although mathematically quadratic, it does not satisfy a class requiring polynomial representation. See [Function](./function.md) for the difference from `g * g`, which constructs an expanded polynomial.

Membership establishes the required representation, not mathematical equivalence or optimality before and after transformation. Those depend on the selected operations and their applicability conditions.

## A class example: QUBO

QUBO (Quadratic Unconstrained Binary Optimization) minimizes a polynomial of degree at most two in Binary variables with no active constraints. {meth}`~ommx.InstanceClass.qubo` returns a class with those representation conditions. It also includes linear and constant objectives.

```{code-cell} ipython3
qubo_instance = Instance.minimize()
a = qubo_instance.new_binary("a")
b = qubo_instance.new_binary("b")
qubo_instance.objective = a + 2 * a * b + 3

qubo_class = InstanceClass.qubo()
assert qubo_class.contains(qubo_instance)

# classに入ったInstanceのactive objectiveを係数と定数項にする。
coefficients, offset = qubo_instance.as_qubo_format()
assert coefficients == {(a.id, a.id): 1.0, (a.id, b.id): 2.0}
assert offset == 3.0
```

QUBO is one class example, not a foundational model separate from an OMMX Instance. Adapter names and classes are not one-to-one either. For example, OpenJij SA accepts Binary polynomials beyond degree two. Use the Adapter's `INPUT_CLASS` instead of assuming QUBO from its name.

**Adapting to a class and formatting backend coefficients are also separate.** `as_qubo_format()` extracts active-objective coefficients and an offset without Preparation. These do not replace the whole Instance, including value-reconstruction rules and removed constraints. See {meth}`~ommx.Instance.as_qubo_format` for exact formatting conditions and [PreparationPolicy](./preparation_policy.md) for a convenience path that also performs Preparation.

## Detail: membership concerns active mathematical content

Membership considers decision variables actually used by the objective and four active constraint families: ordinary, Indicator, OneHot, and SOS1. Variables outside {attr}`~ommx.Instance.used_decision_variables` do not enter variable-kind checks, including:

- Fixed, dependent, or irrelevant variables
- Variables referenced only by removed constraints
- Variables referenced only by named functions
- Variables referenced only by `output_objective`

See [Variable roles, reconstruction, and defaulting](./variable_roles.md) for retained versus currently used variables.

Likewise, removed constraints do not enter relation, degree, or special-constraint checks. An Instance with special constraints lowered to ordinary constraints can retain the sources as removed while belonging to a class that forbids active special constraints. See [Transformed problems and removed constraints](./removed_constraints.md) for active and removed semantics.

## Application to Adapters: INPUT_CLASS and applicability

Every OMMX Adapter must declare `INPUT_CLASS`: the Instances it accepts directly without Preparation. Its type is also InstanceClass. This is not a capability covering everything transformable into an accepted form; it specifies the Instance supplied to strict execution itself.

`INPUT_CLASS` membership is the complete condition for Adapter applicability.

{meth}`~ommx.adapter.SolverAdapter.check_applicability` returns the structured report from `INPUT_CLASS.check_membership()`. {meth}`~ommx.adapter.SolverAdapter.require_applicable` checks the same membership and raises {class}`~ommx.adapter.AdapterNotApplicableError` for a non-member.

Membership does not guarantee that every converter or backend operation succeeds. Implementation limits or numerical conditions, for example, can prevent solver-input construction or execution. These are conversion or backend errors, not additional applicability conditions.

The checks do not include:

- Preparation such as lowering, encoding, or penalty methods
- Backend execution
- A guarantee that the backend returns solutions or samples

Ordinary `solve()` and `sample()` copy the input, prepare it with the recommended Policy, and call strict execution after reaching `INPUT_CLASS` membership. They do not change the caller's Instance.

In contrast, `solve_without_preparation()`, `sample_without_preparation()`, and Adapter constructors do not perform Preparation. Their input must already belong to `INPUT_CLASS`. See [Choose constraint families for Preparation](../how_to/customize_preparation.md) for a complete procedure using an application-specific Policy.
