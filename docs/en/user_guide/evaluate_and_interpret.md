(evaluate-and-interpret)=
# Evaluate candidates and interpret results

You may already have a candidate from a solver, a sampler, or a manually prepared plan. OMMX lets you evaluate it against a common problem representation: which conditions it satisfies, what objective value it has, and what other metrics it produces. You can use this evaluation without running an OMMX Adapter.

**Choose what problem you are evaluating against before comparing results.** A candidate can satisfy the formulation given to a solver while violating a condition that was removed during preparation. Reconstructing its variable values, checking feasibility, and establishing optimality answer different questions.

## Separate the problem being solved from the conditions being checked

Consider maximizing a binary variable $x$ subject to $x \leq 0$. If you remove that constraint, $x=1$ is allowed in the new problem, but violates the original condition.

OMMX can retain a constraint after removing it from the conditions imposed on the solver. The imposed constraints are called **active**; the retained ones are called **removed**. Evaluation can therefore distinguish satisfying the active conditions from satisfying all retained conditions. Variable domains, such as integrality and bounds, matter in both cases.

Removed constraints also arise when a special constraint is replaced by an equivalent expression using ordinary constraints, or when a constraint is converted into a penalty term in the objective. A removed constraint is a record of a change; its presence alone does not tell you whether that change preserves the original problem. In particular, a finite penalty weight can allow an infeasible candidate to be preferred.

Use the transformed Instance—OMMX's representation of the problem associated with the solve—to interpret the returned values and retained conditions. Check the transformations' guarantees before treating that evaluation as a claim about the original problem: retained information does not certify that every change preserved its feasible region or optima.

The {class}`Solution Reference <ommx.Solution>` explains the feasibility results and how to inspect violations. The {attr}`removed-constraint Reference <ommx.Instance.removed_constraints>` covers the retained data and its relationship to active constraints.

## Distinguish reconstructed values from filled-in values

Preparation may fix a variable or express it in terms of other variables, leaving the solver fewer values to choose. The recorded fixed value or expression lets OMMX reconstruct the omitted value. For example, if a transformation records $y=2z$, a returned $z=1$ gives $y=2$; this calculation alone says nothing about whether every original condition is satisfied or every original candidate can be represented.

A variable that is irrelevant to the active problem is different. If its value is omitted, OMMX can fill it with a default. That value was neither selected by the solver nor recovered from the original candidate. It can still affect a retained constraint or a reporting metric. Supply the candidate's value when that distinction matters to your interpretation.

Missing values required by the active problem are not filled with arbitrary defaults. The {meth}`state reconstruction Reference <ommx.Instance.populate_state>` owns the roles, default rules, consistency checks, and executable examples.

## Read the reported objective and solver status in context

A solver's objective and the objective reported to the user can differ. Maximizing $3x$ can be expressed as minimizing $-3x$: the same $x=1$ then has solver-facing value $-3$ and original value $3$. When an output objective is retained, OMMX evaluates that objective for the result. The {attr}`output-objective Reference <ommx.Instance.output_objective>` explains the exact behavior.

With a penalty formulation, the reported objective can likewise exclude the penalty used by the solver. A favorable reported value does not establish feasibility. Inspect the conditions separately.

For example, replace the constraint $x=1$ in a binary minimization of $x$ with the penalty $0.5(x-1)^2$. At $x=0$, the solver-facing objective is $0.5$ and the original objective is $0$; at $x=1$, both are $1$. The penalty formulation prefers the infeasible candidate. Comparing the reported objectives alone, or treating them as the solver's penalty energy, would miss this distinction.

Candidate evaluation also does not establish optimality. A solver may stop at a time limit with a feasible candidate; a candidate selected from many samples may be the best observed without being globally optimal. A solver's optimality claim concerns the formulation it solved, and transporting that claim requires the relevant transformation guarantees. See {meth}`optimality transport <ommx.Instance.map_active_optimality>`.

Backend diagnostics answer another question: why did the solver stop, and what progress did it report? Bounds constrain the optimum of the solver's formulation, and a gap reports a separation according to that backend's definition. Neither is automatically on the scale of the reported output objective after sign, scale, or penalty changes. For concrete report formats, see the {class}`SCIP <ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer>` and {class}`HiGHS <ommx_highs_adapter.HighsDiagnosticsAnalyzer>` diagnostic references.

## Compare candidates using the same criteria

Common evaluation makes results from different methods easier to compare, provided you align the criteria:

- The problem version and numeric conditions, with a consistent correspondence between candidate values and variables.
- The constraints, variable domains, and numerical tolerances used to decide feasibility.
- The objective, optimization direction, and treatment of penalties used for ranking.

{class}`SampleSet <ommx.SampleSet>` supports evaluating and selecting among supplied candidates; its best candidate is best within that set under the chosen criteria. Comparing solving methods additionally requires their execution conditions, such as time limits, to be considered separately. The {mod}`Adapter comparison Reference <ommx.adapter>` provides a complete example of solving one model with HiGHS and SCIP, comparing the results, and plotting the selected values.

For business decisions, you may also need costs, distances, or other metrics beyond the objective. {class}`Named functions <ommx.NamedFunction>` evaluate those metrics without changing what is optimized. {meth}`Named result extraction <ommx.Solution.extract_decision_variables>` returns values using business-facing labels. These capabilities help bring a result back to its application; they do not add a feasibility or optimality guarantee.
