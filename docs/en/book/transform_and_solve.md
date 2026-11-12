(transform-and-solve)=
# Transform a problem and connect it to a solving method

A model expresses the requirements of an application; a solver or sampler searches for candidates. OMMX puts a common problem representation, an **Instance**, between them so that transformations can be shared across modeling tools and solving methods. You can use that common processing even when you keep the same solver, or when you need to transform a problem without solving it yet.

**Choose both the representation you need and the changes in meaning you permit.** A problem that a backend can read is not necessarily equivalent to the problem you started with. Reconstructing a candidate's variables, preserving feasibility, and preserving optimality are separate guarantees.

(keep-meaningful-structure-until-you-choose-how-to-use-it)=
## Choose the formulation to send to the solver

A production line can make up to 100 units per hour, but only 60 in reduced mode. Let $q$ be its hourly output and let $r$ be 1 in reduced mode and 0 otherwise. An **indicator constraint** enforces $q\leq60$ when $r=1$. The PySCIPOpt Adapter can pass this linear indicator directly to SCIP.

To try a formulation with ordinary linear constraints in the same solver, OMMX can {meth}`rewrite the rule as a linear inequality <ommx.Instance.convert_indicator_to_constraint>`, $q\leq100-40r$. At $r=0$ this gives the known upper bound of 100. At $r=1$ it gives 60. The difference of 40 comes from the two limits. If the line could actually produce more than 100 in normal mode, using 100 here would exclude valid plans. Keeping the indicator in the source problem lets you choose the solver formulation later.

The indicator states explicitly which operating mode activates the limit. The inequality expresses the same rule through variables and coefficients. This is a **lower level of abstraction**. With a valid upper bound of 100, both formulations allow exactly the same combinations of mode and output. This is the sense of `lower` in {meth}`~ommx.Instance.lower_special_constraints`, which rewrites selected families of special constraints as ordinary constraints.

When the solver supports special constraints directly, you may also want to make structure explicit. For example, OMMX can {meth}`express a group of linear constraints as an SOS1 constraint <ommx.Instance.promote_sos1_big_m>` when they match a supported pattern. The solver can then use the explicit condition that at most one member may be nonzero.

(adapter-responsibilities)=
## Responsibilities an adapter must uphold

An adapter takes the Instance's **Active Formulation**: its active decision variables and their domains, active constraints, objective, and optimization direction. The Instance maintains which variables belong to this formulation from references in the objective and active constraints. Retained fixed or dependent variables and inactive constraints provide context for interpreting results.

The adapter is responsible for the mathematical correspondence between this formulation and a backend's inputs and outputs. OMMX requires the following contract:

- State which problems it accepts, and preserve the meaning of their conditions and objective when passing them to the backend. It must not silently omit unsupported constraints.
- Map candidate values to OMMX variables and evaluate them against the same mathematical problem that was passed to the backend.
- Interpret candidate availability, termination reasons, and claims such as optimality or infeasibility against the problem actually solved. Failure to find a candidate must not become an infeasibility claim, and candidate evaluation alone must not become an optimality claim.
- Base conclusions about the original problem on the correspondences and preservation guarantees of the transformations used. It must not carry back a conclusion whose required guarantee has not been established.

The backend owns search and its reports; common OMMX processing owns problem transformations, candidate evaluation, and transformation guarantees. The adapter connects them so that the subject and meaning of a report agree. Independent certificate validation is a separate operation from interpreting a report.

Coverage of this contract varies by feature and adapter. Consult the {doc}`individual adapter API references </api/adapters>` for input requirements, supported results and diagnostics, and usage. The {class}`~ommx.adapter.SolverAdapter` and {class}`~ommx.adapter.SamplerAdapter` references define the APIs for implementing adapters.

## Separate input requirements from permitted transformations

An **Adapter** connects an Instance to a backend's input and output and interprets backend reports against the problem actually solved. Its input requirements describe the representation it can accept directly: variable kinds, expression forms, constraint families, and optimization direction. A familiar problem name alone is not enough to decide whether an input matches. Even mathematically equal expressions can have different stored forms.

OMMX calls the process of applying selected transformations toward a target representation **Preparation**. The target says what the result must satisfy; the policy says which transformations may be used. For example, you can choose to express linear indicator constraints as ordinary inequalities and pass SOS1 constraints directly to the solver. Creating a policy does not transform a problem, and reaching the target does not mean every allowed transformation was needed.

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

After execution, interpret the candidate against the corresponding problem and its retained evaluation information. A successful solve call or a favorable backend objective does not replace that check. Even when no candidate is returned, read termination reports against the problem actually solved; carrying conclusions back to the original problem requires the transformations' correspondences and preservation guarantees. [Evaluate candidates and interpret results](./evaluate_and_interpret.md) separates reconstructed values, reported objectives, feasibility, and optimality, and explains conclusions without candidates and how results connect back to requirements.

(shared-analysis-and-transformations)=
## Combine analyses, transformations, and guarantees

A problem that a solver already accepts may still be reduced through preprocessing, or **Presolve**. Selecting and repeating such operations serves a different purpose from Preparation, which establishes input requirements.

OMMX is a system for adding and combining analyses and transformations on a common problem representation. Its operations have the following responsibilities:

- An analysis inspects properties and structure without rewriting the problem and states the conditions on which its results depend.
- A transformation rewrites the problem using analysis results and applicability conditions, and states its correspondence with the original problem and the guarantees it preserves.
- Processing that combines analyses and transformations decides whether analysis results remain reusable after a change and connects transformations according to the required guarantees.

For example, a transformation may use a bound inferred from a constraint. After that constraint becomes inactive, the same bound may no longer be justified. The processing that combines these operations is responsible for checking the analysis dependencies and deciding whether to retain, update, or recompute the result.

New analyses and transformations must define their applicability conditions and guarantees within this division of responsibilities. Users choose the guarantees their task requires; the design makes combinations that satisfy them reusable as common processing. As declaration types and solving methods grow, the contract connecting each transformation to reconstruction and result interpretation remains shared.

Available operations, combinations, and guarantees vary by feature. Consult each operation's API Reference, such as {meth}`Preparation <ommx.Instance.prepare>` or {meth}`bound tightening <ommx.Instance.tighten_bounds_simultaneously_once>`, for its specific applicability conditions and guarantees.
