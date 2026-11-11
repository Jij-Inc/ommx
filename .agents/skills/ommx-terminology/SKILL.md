---
name: ommx-terminology
description: Choose and review OMMX domain terminology when naming APIs, changing their meaning, or explaining model and result semantics. Distinguish formulation membership, evaluation participation, candidate diagnostics, and variable references.
---

# OMMX Terminology

Use this skill when a task adds, renames, or explains OMMX domain concepts.
Apply it to the affected names and passages; an unrelated implementation change
does not require a repository-wide terminology audit.

Book and API Reference own user-facing definitions and detailed contracts.
This skill records naming decisions and the distinctions needed to apply them.
Keep the relevant source documentation current when an accepted design changes;
do not maintain a second catalogue of every API here.

## Choose names by meaning

Before selecting a name, identify its owner and what the value describes:
membership in a formulation, participation in a feasibility check, a configured
condition, an evaluated candidate, or a reference between objects.

| Term | Meaning and naming boundary |
| --- | --- |
| **Active Formulation** | The current solver-facing problem: objective and sense, active decision variables and their domains, and all active constraint families. Membership does not imply that a particular Adapter supports the formulation. |
| **Interpretation Context** | Retained information used to reconstruct values and interpret results, such as inactive constraints, fixed values, dependencies, and an output objective. These are conceptual roles within the Instance, not necessarily disjoint data or a complete copy of the original problem. |
| `active` / `activate` | Membership in the Active Formulation, or moving a retained constraint into it. An active variable is referenced by the current objective or an active constraint, including structural references. Use `active_decision_variables` for this set. |
| `inactive` / `deactivate` | A retained constraint outside the Active Formulation, or moving a constraint out of it while retaining it. Inactive includes the `EVALUATION_ONLY` and `FEASIBILITY_CHECK` roles. Deactivation alone makes no promise of mathematical equivalence. |
| `role` | The role an object has in its Instance. The active role in both `DecisionVariableRole` and `ConstraintRole` means Active Formulation membership; their other roles describe the respective object's use. |
| `ConstraintRole` | `EVALUATION_ONLY` contributes numerical results; `FEASIBILITY_CHECK` additionally participates in normal feasibility; `ACTIVE` additionally belongs to the Active Formulation. These describe participation, not completed checks or candidate feasibility. |
| `skip_feasibility_check` | For an inactive row, `True` selects `EVALUATION_ONLY` and `False` selects `FEASIBILITY_CHECK`. Both roles retain numerical values, violations, and diagnostics. |
| `trigger_value` | The configured Indicator control value, `0` or `1`, at which its body applies; the default is `1`. |
| `indicator_on` | Whether an evaluated candidate satisfies the Indicator trigger condition, according to its numerical contract. It is independent of active/inactive membership. |
| `selected_variable` | OneHot/SOS1 member selection diagnosed from an evaluated candidate. It is independent of Active Formulation membership. |
| `required_ids` | Values the caller must supply for that object's `Evaluate` contract, including any reconstruction performed by that object. It is not a general name for active references. |
| `used_in_*`, row-level `used_decision_variable_ids`, `used_ids` | References within an expression or row, including inactive rows. Keep this relational meaning distinct from formulation membership. |

Evaluation stages (`Created`, `Evaluated`, `Sampled`) and constraint roles are
separate dimensions. A role is defined before evaluation and retained in results;
it does not state whether evaluation has already occurred.

For example, a variable referenced only by an inactive constraint is used by
that row but is not active. A standalone constraint collection requires values
for all rows it evaluates, including `EVALUATION_ONLY` rows. Instance evaluation
first populates fixed, dependent, and omitted irrelevant values, so its
`required_ids` includes only active inputs. Use `active_decision_variable_ids`
when asking about active references themselves.

An inactive Indicator can have `indicator_on=True` for a candidate. Calling
this diagnostic `active` would confuse the candidate's value with formulation
membership. Likewise, use `selected_variable` for OneHot/SOS1 diagnostics.
In mathematical discussion, distinguish a tight or binding inequality from
OMMX's formulation-membership use of `active`.

## Make query scope explicit

Model `constraints` views include every role; `active_constraints`
and `inactive_constraints` identify filtered views. Name an active-only query
accordingly. Verify the actual family and state scope before renaming a query;
an all-state result query should not gain an `active_` prefix.

The order is `EVALUATION_ONLY < FEASIBILITY_CHECK < ACTIVE`.
`min_role` is an inclusive lower bound, not an exact-role selector:
`FEASIBILITY_CHECK` includes rows with that role and active rows;
`EVALUATION_ONLY` includes every retained row. This participation order belongs
to `ConstraintRole`; it does not give `DecisionVariableRole` categories such as
`Fixed` and `Dependent` an equivalent threshold meaning.

Keep the feasibility judgment separate from a selected violation aggregate:
`feasible` checks constraints with roles `FEASIBILITY_CHECK` and `ACTIVE`;
`feasible_relaxed` checks active constraints. Both also check variable domains.
Python exposes these as properties without a `min_role` argument.
`total_violation` and `constraints_df` accept that selector; their defaults
are `FEASIBILITY_CHECK` and `EVALUATION_ONLY`, respectively.
Matching the constraint set does not make exact-zero total violation equivalent
to feasibility, which also applies tolerances and domain checks.

## Apply the policy to a change

Follow the meaning across the affected Rust API, Python bindings, generated
exports and stubs, DataFrame columns, wire schema, and documentation. Regenerate
derived surfaces from their owners. This is a scoped check, not a requirement
to modify every surface for every wording change.

Distinguish current domain terminology from wire-format names, migration
examples, historical quotations, and ordinary language. For example, v1
`RemovedConstraint` is a wire name; removing a row entirely is different from
deactivating it. Check the applicable format contract before renaming fields.
Do not globally replace every occurrence of `removed`, `used`, or `active`.

When existing prose and implementation disagree, resolve the intended domain
meaning from the accepted design and its owner. Do not silently redefine a term
to match an accidental implementation or expand a naming task into a behavior
change.

## Definition owners

Read the relevant source for exact behavior rather than copying its full
contract into this skill:

- [Transform and solve](../../../docs/en/book/transform_and_solve.md) and
  [evaluate and interpret](../../../docs/en/book/evaluate_and_interpret.md):
  the Book explanation of formulation scope and result interpretation.
- [Instance Rustdoc](../../../rust/ommx/src/instance.rs), section
  "Active Formulation and Interpretation Context", and
  [variable analysis](../../../rust/ommx/src/instance/analysis.rs):
  model roles and the active variable set.
- [Inactive constraint Reference](../../../python/ommx/src/docstrings/ommx/Instance.inactive_constraints.md):
  retention, evaluation, and feasibility participation.
- [Constraint roles](../../../rust/ommx/src/constraint/stage.rs): participation
  order and its relationship to inactive metadata.
- [Evaluate](../../../rust/ommx/src/evaluate.rs): the `required_ids` input contract.
- [Indicator](../../../rust/ommx/src/indicator_constraint/mod.rs) and
  [Python result bindings](../../../python/ommx/src/constraint_results.rs):
  trigger configuration and candidate diagnostics.
- [Solution Reference source](../../../python/ommx/src/solution.rs):
  canonical feasibility and total violation.
