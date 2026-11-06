(evaluate-and-interpret)=
# Evaluate candidates and interpret results

You may already have a candidate from a solver, a sampler, or a manually prepared plan. OMMX lets you evaluate it against a common problem representation: which conditions it satisfies, what objective value it has, and what other metrics it produces. You can use this evaluation without running an OMMX Adapter.

**Choose what problem you are evaluating against before comparing results.** A candidate can satisfy the formulation given to a solver while violating a condition that was removed during preparation. Reconstructing its variable values, checking feasibility, and establishing optimality answer different questions.

When a solve returns no candidate, read its termination report to determine what was established. Connecting the conditions identified by candidates or diagnostics back to requirements and input data helps you decide where to revise a plan, requirement, or data value.

## Separate the problem being solved from the conditions being checked

A production line can make up to 100 units per hour, but only 60 in reduced mode. Suppose a planner proposes producing 65 units per hour. In normal mode the proposal satisfies this production limit. In reduced mode it exceeds the limit by 5, even if a solver was allowed to ignore the reduced-mode limit while searching. The mode and output together determine the verdict.

You can apply the same check to a candidate from a solver or one supplied by hand. Passing this one check still leaves the other constraints and variable domains to check. Even a plan that satisfies them all might have a worse objective value than another feasible plan.

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

Backend diagnostics answer another question: why did the solver stop, and what progress did it report? Bounds constrain the optimum of the solver's formulation, and a gap reports a separation according to that backend's definition. Neither is automatically on the scale of the reported output objective after sign, scale, or penalty changes.

For example, in minimization, a best feasible objective of 120 and a proven lower bound of 100 place the optimum between 100 and 120. These are the primal and dual bounds respectively; for maximization, their upper/lower roles reverse. A progress graph shows how the search narrows this interval. A solver can stop before the bounds meet, and finite bounds may be unavailable before it finds a candidate. The reported relative gap follows the backend's definition, so it is not simply the interval width.

For collection, field meanings, and executable examples, see the {class}`SCIP <ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer>` and {class}`HiGHS <ommx_highs_adapter.HighsDiagnosticsAnalyzer>` diagnostic references. The SCIP Reference illustrates how the bounds converge in a recorded solve. Intermediate records are observations; use the termination report to read the final outcome.

## Read what was established when no candidate is returned

Finding no candidate before a time limit calls for a different decision from concluding that no candidate satisfies the conditions. First identify the problem the solver actually solved and read its termination report.

| Report | What it says about that problem |
|---|---|
| No candidate returned | No candidate was obtained; this does not prove infeasibility |
| Infeasible | It was concluded that no feasible solution exists |
| Unbounded | It was concluded that feasible objective values can improve without limit |
| Infeasible or unbounded | One of these was reported, without distinguishing which |

An unknown status or a time limit alone does not establish infeasibility or unboundedness. A time limit can also leave a candidate available. Input conversion failures and solver execution failures are separate from mathematical infeasibility. See the Reference for {class}`~ommx.adapter.NoSolutionReturned`, {class}`~ommx.InfeasibleDetected`, {class}`~ommx.adapter.UnboundedDetected`, and {class}`~ommx.InfeasibleOrUnboundedDetected` for the exception meanings.

An adapter interprets backend reports against the input actually solved. It also decodes and evaluates candidates against an Instance mathematically identical to that input. The {class}`~ommx.adapter.SolverAdapter` contract explains this correspondence and why evaluation alone does not set optimality. Mapping a report to an OMMX result does not itself independently validate a certificate.

Whether a conclusion carries back to the original problem depends on what each transformation preserves. For example, a subproblem that fixes output to 65 units and selects reduced mode contradicts the limit of 60. This does not establish infeasibility of the original problem, where output and mode remain choices. Similarly, infeasibility after adding a cost threshold does not imply infeasibility without that extra condition.

## Compare candidates using the same criteria

Common evaluation makes results from different methods easier to compare, provided you align the criteria:

- The problem version and numeric conditions, with a consistent correspondence between candidate values and variables.
- The constraints, variable domains, and numerical tolerances used to decide feasibility.
- The objective, optimization direction, and treatment of penalties used for ranking.

{class}`SampleSet <ommx.SampleSet>` supports evaluating and selecting among supplied candidates; its best candidate is best within that set under the chosen criteria. Comparing solving methods additionally requires their execution conditions, such as time limits, to be considered separately. The {mod}`Adapter comparison Reference <ommx.adapter>` provides a complete example of solving one model with HiGHS and SCIP, comparing the results, and plotting the selected values.

For business decisions, you may also need costs, distances, or other metrics beyond the objective. {class}`Named functions <ommx.NamedFunction>` evaluate those metrics without changing what is optimized. {meth}`Named result extraction <ommx.Solution.extract_decision_variables>` returns values using business-facing labels. These capabilities help bring a result back to its application; they do not add a feasibility or optimality guarantee.

## Connect results back to requirements and input data

Suppose you convert the reduced-mode limit into an ordinary inequality. With output $q$ and a variable $r$ equal to 1 in reduced mode, the normal-mode upper bound of 100 gives $q\leq100-40r$. The candidate $q=65, r=1$ violates this inequality. Tracing the generated constraint's provenance connects it to the original indicator constraint: at most 60 units in reduced mode.

In OMMX, labels identify mathematical objects, and {class}`provenance <ommx.Provenance>` connects a generated ordinary constraint to its source special constraint. {attr}`Solution constraint rows <ommx.Solution.constraints>` retain labels, provenance, removal reasons, and evaluation information. The {meth}`special-constraint lowering example <ommx.Instance.lower_special_constraints>` shows how to inspect source and generated constraints.

To identify the corresponding business declaration or input data, combine this information with the mapping maintained by a modeler such as JijModeling or by your application. For example, a constraint label can identify the production line and time period whose planned output, operating mode, capacity data, or requested deadline needs inspection. The user then decides whether to revise the plan, correct an input error, or reconsider a requirement.

Tracing an origin, checking a candidate's violation, and establishing that the requirements cannot all hold are separate tasks. This candidate's limit violation and provenance alone do not establish infeasibility or identify conflicting requirements. Special-constraint provenance provides a foundation today. Extending the correspondence through multiple transformations, from results and diagnostics to high-level declarations and their data, remains a development direction.
