(transform-and-solve)=
# Transform a problem and connect it to a solving method

A model expresses the requirements of an application; a solver or sampler searches for candidates. OMMX puts a common problem representation, an **Instance**, between them so that transformations can be shared across modeling tools and solving methods. You can use that common processing even when you keep the same solver, or when you need to transform a problem without solving it yet.

**Choose both the representation you need and the changes in meaning you permit.** A problem that a backend can read is not necessarily equivalent to the problem you started with. Reconstructing a candidate's variables, preserving feasibility, and preserving optimality are separate guarantees.

## Keep meaningful structure until you choose how to use it

Suppose two amounts satisfy $0\leq x,y\leq10$, and at most one may be nonzero. OMMX can keep this condition as an SOS1 constraint, with a meaning defined independently of a solver. For a supported model, the PySCIPOpt Adapter can pass that structure directly to SCIP. For HiGHS, OMMX can instead express it using Binary selectors and ordinary linear constraints, provided the conversion conditions and the adapter's input requirements hold.

Writing the condition once leaves this choice available later. The modeling tool need not implement a different conversion for each backend, and the conversion can be reused by another solving method. Keeping the original condition also helps interpret the resulting candidate in the terms of the model.

Rewriting a special constraint as ordinary constraints is called **lowering**. The reverse direction is useful too: OMMX can verify selected patterns of ordinary constraints and represent them as an SOS1 constraint through **promotion**. These are supported operations with specific conditions, rather than automatic recognition of every equivalent formulation. Their definitions, examples, and conversion contracts belong to the {class}`SOS1 Reference <ommx.Sos1Constraint>` and {meth}`promotion Reference <ommx.Instance.promote_sos1_big_m>`.

## Separate input requirements from permitted transformations

An **Adapter** connects an Instance to a backend's input and output. Its input requirements describe the representation it can accept directly: variable kinds, expression forms, constraint families, and optimization direction. A familiar problem name alone is not enough to decide whether an input matches. Even mathematically equal expressions can have different stored forms.

OMMX calls the process of applying selected transformations toward a target representation **Preparation**. The target says what the result must satisfy; the policy says which transformations may be used. A policy can allow one special-constraint family to be lowered while keeping another explicit. Creating a policy does not transform a problem, and reaching the target does not mean every allowed transformation was needed.

The {class}`input-class Reference <ommx.InstanceClass>` explains how to inspect the representation, and {meth}`Instance.prepare <ommx.Instance.prepare>` provides a complete transformation and candidate-evaluation example. A successful preparation establishes the requested input conditions. The mathematical correspondence with the original problem comes from the transformations used; converter or backend execution may still fail afterward.

QUBO minimizes a polynomial of degree at most two over Binary variables without constraints imposed on the solver. It is one possible target representation, not a separate foundation for OMMX or a synonym for every sampling backend. Preparing a QUBO still produces an OMMX Instance. Exporting only its coefficients and constant term crosses a different boundary: those numbers do not carry the retained constraints or rules for reconstructing variable values. See the {meth}`QUBO preparation policy <ommx.PreparationPolicy.for_qubo>` and {meth}`coefficient-format Reference <ommx.Instance.as_qubo_format>` for the two operations.

## Choose the guarantees needed by your task

Different transformations serve different purposes. Before composing them, decide which correspondence your application needs.

| Operation | Question for the application |
|---|---|
| Rewrite a condition using another expression | Under what domain and bound conditions does it express the same requirement? |
| Encode an integer using Binary variables | Can the required values be represented, and how are original values reconstructed? |
| Tighten bounds using constraints | Does the update follow from the problem's conditions, with the operation's numerical guarantees? |
| Fix a variable | Is the fixed value implied by the problem, or are you choosing a conditional subproblem? |
| Relax a constraint | Must a candidate from the larger feasible region also satisfy the omitted condition? |
| Replace a constraint with a penalty | What is penalized, and are the formulation and weight sufficient for the intended result? |

For example, minimizing a Binary variable $x$ has optimum $0$. If you choose to fix $x=1$, the conditional subproblem has optimum $1$. Restoring the fixed value gives a feasible original candidate, but not an original optimum. In contrast, fixing a value already implied by the original constraints can remove work without making this extra restriction. The reason for the fixing matters; the {meth}`partial-evaluation Reference <ommx.Instance.partial_evaluate>` owns the operation and its executable example.

A penalty requires care about the expression as well as the weight. Consider minimizing $x$ subject to $x-1\leq0$ with $x\in\{0,1\}$. Both values are feasible. Squaring the inequality body and adding weight $2$ changes the objective to $x+2(x-1)^2$:

| $x$ | Original objective | Squared body | Objective with penalty |
|---|---|---|---|
| 0 | 0 | 1 | 2 |
| 1 | 1 | 0 | 1 |

This reverses the order of two feasible candidates. It differs from penalizing only the violation, $\max(0,x-1)^2$, which is zero for both. Increasing the weight alone therefore does not guarantee recovery of the original optimum. OMMX's {meth}`fixed-penalty operation <ommx.FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight>` squares constraint bodies; its Reference explains the contract and links to converting suitable inequalities into equalities with slack variables.

The required guarantee also depends on the task. Preserving an optimal value need not preserve every feasible candidate or a sampling distribution. Check the correspondence of candidates, feasibility in both directions, objective values, and optimality over the set you care about. Numerical tolerances are part of the individual operation's contract; agreement on a few sample candidates is not a proof for every input.

## Choose a method, settings, and a way forward

An adapter's recommended policy is a starting point. The remaining decisions belong to the application:

- Which representation the method accepts directly, and which conversions are needed to reach it.
- Which changes in meaning are acceptable, including fixing, relaxation, or penalty choices.
- Which settings to supply, such as penalty weights, numerical tolerances, sample counts, or time limits.
- Which result is useful: one candidate or several, and what feasibility, termination, bound, gap, or optimality information the backend provides.

If preparation cannot reach the target, inspect the unmet input conditions and the chosen operations. You may need a backend that accepts the retained structure, additional valid bounds for a conversion, or a different model or transformation policy. Permitting a relaxation or choosing a penalty is a mathematical decision; it does not follow merely from a request to make the input acceptable. The {meth}`Preparation Reference <ommx.Instance.prepare>` explains failures and which changes may remain in the Instance.

The {mod}`Adapter Reference <ommx.adapter>` explains the execution paths for recommended or explicitly chosen preparation. The {doc}`adapter catalogue </api/adapters>` leads to each backend's contracts. Solver names are not interchangeable with problem classes: for example, OpenJij SA can accept Binary polynomials beyond quadratic degree, so its actual input requirements are the useful starting point.

After execution, interpret the candidate against the corresponding problem and its retained evaluation information. A successful solve call or a favorable backend objective does not replace that check. [Evaluate candidates and interpret results](./evaluate_and_interpret.md) explains how to separate reconstructed values, reported objectives, feasibility, and optimality when comparing results.

## Build on common processing

OMMX already provides individual transformations, policies that select supported operations, and candidate evaluation. It also provides {meth}`bound tightening <ommx.Instance.tighten_bounds_simultaneously_once>`: for example, a constraint $x\leq3$ can refine a declared upper bound of $10$. This operation applies one simultaneous update; it is not a complete presolver that repeats until convergence or detects every infeasible problem.

The development direction is to accumulate both transformation techniques and the knowledge of when to select and repeat them. Keeping these operations on the common representation lets new modelers and solving methods reuse that work. Today, choose the supported operations and read their guarantees; general automatic orchestration remains a development direction.
