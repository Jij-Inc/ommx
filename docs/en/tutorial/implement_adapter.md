---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# Implementing an OMMX Adapter

As mentioned in [Solve with multiple adapters and compare the results](../tutorial/switching_adapters), OMMX Adapters have a common API. This common API is realized by inheriting the abstract base classes provided by the OMMX Python SDK. OMMX provides two abstract base classes depending on the type of adapter:

- [`ommx.adapter.SolverAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SolverAdapter): An abstract base class for optimization solvers that return one solution
- [`ommx.adapter.SamplerAdapter`](https://jij-inc.github.io/ommx/python/ommx/autoapi/ommx/adapter/index.html#ommx.adapter.SamplerAdapter): An abstract base class for sampling-based optimization solvers

Solvers that produce multiple solutions can be automatically treated as solvers returning a single solution by selecting the best sample. Therefore, `SamplerAdapter` inherits `SolverAdapter`. If you are unsure which one to implement, consider the number of solutions: if the solver returns one solution, use `SolverAdapter`; if it returns multiple solutions, use `SamplerAdapter`. For example, exact solvers like [PySCIPOpt](https://github.com/scipopt/PySCIPOpt) should use `SolverAdapter`, while samplers like [OpenJij](https://github.com/OpenJij/OpenJij) should use `SamplerAdapter`.

In OMMX, a class inheriting `ommx.adapter.SolverAdapter` is called a **Solver Adapter** and one inheriting `ommx.adapter.SamplerAdapter` is called a **Sampler Adapter**.
For clear explanation in this chapter, the software that the adapter wraps (such as PySCIPOpt or OpenJij) is referred to as "backend solver".

## Adapter Workflow

The adapter process can be roughly divided into these 3 steps:

1. Convert `ommx.Instance` into a format the backend solver can understand
2. Run the backend solver to obtain a solution
3. Convert the backend solver's output into `ommx.Solution` or `ommx.SampleSet`

Because step 2 is nothing but the usage of the backend solver, we assume you are familiar with it. This tutorial explains steps 1 and 3.

Many backend solvers are designed to receive only the minimum necessary information to represent an optimization problem in a form suitable for their algorithms, whereas `ommx.Instance` contains more information, assuming optimization as part of data analysis. Therefore, step 1 involves discarding much of this information. Additionally, OMMX manages decision variables and constraints with IDs that are not necessarily sequential, while some backend solvers manage them by names or sequential numbers. This correspondence is needed in step 3, so the adapter must manage it.

Conversely, in step 3, `ommx.Solution` or `ommx.SampleSet` cannot be constructed solely from the backend solver's output. Instead, the adapter will construct `ommx.State` or `ommx.Samples` from the backend solver's output and the information from step 1, then convert it to `ommx.Solution` or `ommx.SampleSet` using `ommx.Instance`.

## Implementing a Solver Adapter

Here, we will implement a Solver Adapter using PySCIPOpt as an example. For a complete example, refer to [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter).

For this tutorial, we will proceed in the following order to make it easier to execute step by step:

- Implement functions to construct a PySCIPOpt model from `ommx.Instance` one by one.
- Finally, combine these functions into the `OMMXPySCIPOptAdapter` class.

### Custom Exception

First, it is good to define custom exceptions. This makes it easier for users to understand which part is causing the problem when an exception occurs.

```{code-cell} ipython3
class OMMXPySCIPOptAdapterError(Exception):
    pass
```

OMMX can store a wide range of optimization problems, so the adapter declares the exact set it accepts with `INPUT_CLASS`, as shown later. The converter helpers below may still validate the representation they consume and raise while building solver input. Such converter or backend failures are not additional applicability conditions: adapter applicability is defined only by `INPUT_CLASS` membership.

### Setting Decision Variables

PySCIPOpt manages decision variables by name, so register the OMMX decision variable IDs as strings. This allows you to reconstruct `ommx.State` from PySCIPOpt decision variables in the `decode_to_state` function mentioned later. Note that the appropriate method depends on the backend solver's implementation. The important thing is to retain the information needed to convert to `ommx.State` after obtaining the solution.

```{code-cell} ipython3
import pyscipopt
from ommx import Instance, Solution, DecisionVariable, Constraint, State, Function

def set_decision_variables(
    model: pyscipopt.Model,  # For tutorial purposes, we pass state as arguments, but managing with class is common
    instance: Instance
) -> dict[str, pyscipopt.Variable]:
    """
    Add decision variables to the model and create a mapping from variable names to variables
    """
    # Create PySCIPOpt variables from OMMX decision variable information
    for var in instance.used_decision_variables:
        if var.kind == DecisionVariable.BINARY:
            model.addVar(name=str(var.id), vtype="B")
        elif var.kind == DecisionVariable.INTEGER:
            model.addVar(
                name=str(var.id), vtype="I", lb=var.bound.lower, ub=var.bound.upper
            )
        elif var.kind == DecisionVariable.CONTINUOUS:
            model.addVar(
                name=str(var.id), vtype="C", lb=var.bound.lower, ub=var.bound.upper
            )
        else:
            # Throw an error if an unsupported decision variable type is encountered
            raise OMMXPySCIPOptAdapterError(
                f"Unsupported decision variable kind: "
                f"id: {var.id}, kind: {var.kind}"
            )

    # If the objective is quadratic, add an auxiliary variable for linearization
    if instance.objective.degree() == 2:
        model.addVar(
            name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
        )

    # Create a dictionary to access the variables added to the model
    return {var.name: var for var in model.getVars()}
```

### Converting `ommx.Function` to `pyscipopt.Expr`

Implement a function to convert `ommx.Function` to `pyscipopt.Expr`. Since `ommx.Function` only has the OMMX decision variable IDs, you need to obtain the PySCIPOpt variables from the IDs using the variable name and variable mapping created in `set_decision_variables`.

```{code-cell} ipython3
def make_linear_expr(function: Function, varname_map: dict) -> pyscipopt.Expr:
    """Helper function to generate a linear expression"""
    return (
        pyscipopt.quicksum(
            coeff * varname_map[str(id)]
            for id, coeff in function.linear_terms.items()
        )
        + function.constant_term
    )


def make_quadratic_expr(function: Function, varname_map: dict) -> pyscipopt.Expr:
    """Helper function to generate a quadratic expression"""
    quad_terms = pyscipopt.quicksum(
        varname_map[str(row)] * varname_map[str(col)] * coeff
        for (row, col), coeff in function.quadratic_terms.items()
    )

    linear_terms = pyscipopt.quicksum(
        coeff * varname_map[str(var_id)]
        for var_id, coeff in function.linear_terms.items()
    )

    constant = function.constant_term

    return quad_terms + linear_terms + constant
```

### Setting Objective Function and Constraints

Add the objective function and constraints to the `pyscipopt.Model`. This part requires knowledge of what and how the backend solver supports. For example, in the following code, since PySCIPOpt cannot directly handle quadratic objective functions, an auxiliary variable is introduced according to the [PySCIPOpt documentation](https://pyscipopt.readthedocs.io/en/latest/tutorials/expressions.html#non-linear-objectives).

```{code-cell} ipython3
import math

def set_objective(model: pyscipopt.Model, instance: Instance, varname_map: dict):
    """Set the objective function for the model"""
    objective = instance.objective

    if instance.sense == Instance.MAXIMIZE:
        sense = "maximize"
    elif instance.sense == Instance.MINIMIZE:
        sense = "minimize"
    else:
        raise OMMXPySCIPOptAdapterError(
            f"Sense not supported: {instance.sense}"
        )

    degree = objective.degree()
    if degree == 0:
        model.setObjective(objective.constant_term, sense=sense)
    elif degree == 1:
        expr = make_linear_expr(objective, varname_map)
        model.setObjective(expr, sense=sense)
    elif degree == 2:
        # Since PySCIPOpt doesn't support quadratic objectives directly, linearize using an auxiliary variable
        auxilary_var = varname_map["auxiliary_for_linearized_objective"]

        # Set the auxiliary variable as the objective
        model.setObjective(auxilary_var, sense=sense)

        # Add a constraint for the auxiliary variable
        expr = make_quadratic_expr(objective, varname_map)
        if sense == "minimize":
            constr_expr = auxilary_var >= expr
        else:  # sense == "maximize"
            constr_expr = auxilary_var <= expr

        model.addCons(constr_expr, name="constraint_for_linearized_objective")
    else:
        raise OMMXPySCIPOptAdapterError(
            "The objective function must be `constant`, `linear`, or `quadratic`."
        )
        
def set_constraints(model: pyscipopt.Model, instance: Instance, varname_map: dict):
    """Set the constraints for the model"""
    # Process regular constraints. instance.constraints is a dict[int, Constraint]
    # keyed by constraint ID.
    for constraint_id, constraint in instance.constraints.items():
        # Generate an expression based on the type of constraint function
        f = constraint.function
        degree = f.degree()
        if degree == 0:
            # For constant constraints, check feasibility
            constant_value = f.constant_term
            if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                constant_value, 0, abs_tol=1e-6
            ):
                continue
            elif (
                constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                and constant_value <= 1e-6
            ):
                continue
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Infeasible constant constraint was found: id {constraint_id}"
                )
        elif degree == 1:
            expr = make_linear_expr(f, varname_map)
        elif degree == 2:
            expr = make_quadratic_expr(f, varname_map)
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Constraints must be either `constant`, `linear` or `quadratic`. "
                f"id: {constraint_id}, "
                f"degree: {degree}"
            )

        # Add constraints based on the type (equality/inequality)
        if constraint.equality == Constraint.EQUAL_TO_ZERO:
            constr_expr = expr == 0
        elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
            constr_expr = expr <= 0
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Not supported constraint equality: "
                f"id: {constraint_id}, equality: {constraint.equality}"
            )

        # Add the constraint to the model
        model.addCons(constr_expr, name=str(constraint_id))
```

Also, if the backend solver supports special constraints (e.g., [SOS constraints](https://en.wikipedia.org/wiki/Special_ordered_set)), you need to add functions to handle them.

Now, we can construct a `pycscipopt.Model` from `ommx.Instance`.

### Converting Obtained Solutions to `ommx.State`

Next, implement a function to convert the solution obtained by solving the PySCIPOpt model to `ommx.State`. First, check if it is solved. SCIP has functions to guarantee optimality and detect unbounded solutions, so throw corresponding exceptions if detected. This also depends on the backend solver.

```{warning}
Note that `ommx.adapter.InfeasibleDetected` means that the optimization problem itself is infeasible, i.e., **it is guaranteed to have no solutions**. Do not use this when a heuristic solver fails to find any feasible solutions.
```

```{code-cell} ipython3
from ommx.adapter import InfeasibleDetected, UnboundedDetected

def decode_to_state(model: pyscipopt.Model, instance: Instance) -> State:
    """Create an ommx.State from an optimized PySCIPOpt Model"""
    if model.getStatus() == "unknown":
        raise OMMXPySCIPOptAdapterError(
            "The model may not be optimized. [status: unknown]"
        )

    if model.getStatus() == "infeasible":
        raise InfeasibleDetected("Model was infeasible")

    if model.getStatus() == "unbounded":
        raise UnboundedDetected("Model was unbounded")

    try:
        # Get the best solution
        sol = model.getBestSol()
        # Create a mapping from variable names to variables
        varname_map = {var.name: var for var in model.getVars()}
        # Create a State with a mapping from variable IDs to their values
        return State(
            entries={
                var.id: sol[varname_map[str(var.id)]]
                for var in instance.used_decision_variables
            }
        )
    except Exception:
        raise OMMXPySCIPOptAdapterError(
            f"There is no feasible solution. [status: {model.getStatus()}]"
        )
```

### Creating a Class that Inherits `ommx.adapter.SolverAdapter`

Finally, create a class that inherits `ommx.adapter.SolverAdapter` to standardize the API for each adapter. This is an abstract base class with `@abstractmethod` as follows:

```python
from typing import Any, ClassVar

class SolverAdapter(ABC):
    # Complete OMMX-defined condition for adapter applicability.
    INPUT_CLASS: ClassVar[InstanceClass]

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        return PreparationPolicy()

    @classmethod
    @abstractmethod
    def solve_strict(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
        **kwargs: Any,
    ) -> Solution:
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass
```

This abstract base class assumes the following three use cases:

- For the usual one-call workflow, use the inherited `solve` class method. It
  copies the input, applies `recommended_preparation_policy()` for
  `INPUT_CLASS`, and calls `solve_strict` on that temporary prepared copy.
- If the application prepares an exact Adapter input itself, call
  `solve_strict`. This method performs no Preparation.
- If you adjust the backend solver's parameters, use `solver_input` to get the data structure for the backend solver (in this case, `pyscipopt.Model`), adjust it, then input it to the backend solver, and finally convert the backend solver's output using `decode`.

The `solve_strict` class method may define additional adapter-specific keyword options in concrete adapters; `solve` forwards them unchanged. The reserved `diagnostics` keyword is owned by `Run.log_solve`. When `Run.log_solve(..., store_diagnostics=True)` is used, adapters may record adapter-defined diagnostic reports into that sink; `None` means diagnostics are disabled.

#### Input Class and Recommended Preparation

An adapter defines applicability entirely with `INPUT_CLASS`, the set of exact `Instance` values it accepts. `check_applicability()` reports membership without mutating the caller's instance, and `require_applicable()` raises with the same structured report only when membership fails.

Applicability does not promise that every later conversion or backend operation succeeds. A converter may validate the narrower representation handled by a helper such as `as_linear()`, and a backend may reject a numeric value or implementation limit while solver input is being built. Report those as converter or backend errors; do not add them as a second source of adapter applicability semantics.

The usual `solve` API owns Preparation. It copies the caller's Instance,
applies the fresh policy returned by `recommended_preparation_policy()` to the
copy, and invokes `solve_strict`. The caller's Instance is unchanged, including
when Preparation or the backend fails. The recommendation itself does not
inspect or mutate an instance, run Preparation, or guarantee applicability.

`solve_strict` is the preparation-free API. It requires an exact member of
`INPUT_CLASS` and raises `AdapterNotApplicableError` otherwise. Applications
that need a custom Preparation policy explicitly copy their Instance, apply the
edited policy with {meth}`~ommx.Instance.prepare`, and pass that working copy to
`solve_strict`.

A recommendation can enable special-constraint lowering with these family selectors:

- `SpecialConstraintKind.Indicator`: Indicator constraints (`binvar = 1 → f(x) <= 0`)
- `SpecialConstraintKind.OneHot`: Exactly one of a set of binary variables is 1
- `SpecialConstraintKind.Sos1`: At most one of a set of variables is non-zero

Use {attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` to inspect the currently active families. The selected Preparation phase delegates to {meth}`Instance.lower_special_constraints <ommx.Instance.lower_special_constraints>`, which converts each selected active family into regular constraints (Big-M for indicator / SOS1, linear equality for one-hot). The owner operation still defines its validation and mathematical meaning.

```{important}
`INPUT_CLASS` describes the exact value received by `solve_strict`, and
membership is the complete applicability condition. `solve` performs
Preparation only on its private working copy. Successful Preparation guarantees
membership. Building solver input can still fail during converter-local or
backend validation, but that failure does not make the input "not applicable."
```

Using the functions prepared so far, you can implement it as follows:

```{code-cell} ipython3
from typing import Any

from ommx.adapter import DiagnosticsSink, SolverAdapter
from ommx import (
    DegreeBound,
    Equality,
    InstanceClass,
    InstanceClassClause,
    Kind,
    PreparationPolicy,
    Sense,
    SpecialConstraintKind,
    SpecialConstraintPreparation,
)

class OMMXPySCIPOptAdapter(SolverAdapter):
    INPUT_CLASS = InstanceClass(
        [
            InstanceClassClause(
                label="tutorial-quadratic-mip",
                allowed_variable_kinds={Kind.Binary, Kind.Integer, Kind.Continuous},
                objective_degree_bound=DegreeBound.at_most(2),
                regular_constraint_degree_bounds={
                    Equality.EqualToZero: DegreeBound.at_most(2),
                    Equality.LessThanOrEqualToZero: DegreeBound.at_most(2),
                },
                indicator_constraint_degree_bounds={
                    Equality.EqualToZero: DegreeBound.at_most(1),
                    Equality.LessThanOrEqualToZero: DegreeBound.at_most(1),
                },
                allows_sos1=True,
                allowed_senses={Sense.Minimize, Sense.Maximize},
            )
        ]
    )

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        return PreparationPolicy(
            special_constraints=SpecialConstraintPreparation.lower_special_constraints(
                kinds={SpecialConstraintKind.OneHot}
            )
        )

    def __init__(
        self,
        ommx_instance: Instance,
    ):
        self.require_applicable(ommx_instance)
        self.instance = ommx_instance
        self.model = pyscipopt.Model()
        self.model.hideOutput()

        # Build the model with helper functions
        self.varname_map = set_decision_variables(self.model, self.instance)
        set_objective(self.model, self.instance, self.varname_map)
        set_constraints(self.model, self.instance, self.varname_map)

    @classmethod
    def solve_strict(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
        **kwargs: Any,
    ) -> Solution:
        """
        Solve an ommx.Instance using PySCIPopt and return an ommx.Solution
        """
        _ = diagnostics
        adapter = cls(ommx_instance, **kwargs)
        model = adapter.solver_input
        model.optimize()
        return adapter.decode(model)

    @property
    def solver_input(self) -> pyscipopt.Model:
        """Return the generated PySCIPopt model"""
        return self.model

    def decode(self, data: pyscipopt.Model) -> Solution:
        """
        Generate an ommx.Solution from an optimized pyscipopt.Model and the OMMX Instance
        """
        # Check solution status
        if data.getStatus() == "infeasible":
            raise InfeasibleDetected("Model was infeasible")

        if data.getStatus() == "unbounded":
            raise UnboundedDetected("Model was unbounded")

        # Convert the solution to state
        state = decode_to_state(data, self.instance)
        # Evaluate the state using the instance
        solution = self.instance.evaluate(state)

        # Set the optimality status
        if data.getStatus() == "optimal" and self.instance.output_objective is None:
            solution.optimality = Solution.OPTIMAL

        return solution
```

The backend status certifies the active formulation. When Preparation has
installed an `output_objective`, this generic example therefore leaves
`solution.optimality` unspecified. An adapter may transport the certificate
only when its transformation has a corresponding proof.

The usual call copies and prepares the Instance automatically:

```python
solution = OMMXPySCIPOptAdapter.solve(instance)
```

The caller's `instance` is unchanged. To customize Preparation, create an
explicit working copy and invoke the strict API:

```python
import copy

working = copy.copy(instance)
input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
policy = OMMXPySCIPOptAdapter.recommended_preparation_policy()
# Edit public policy fields here when the application needs different choices.
working.prepare(input_class, policy)
solution = OMMXPySCIPOptAdapter.solve_strict(working)
```

`solve_strict()` checks `INPUT_CLASS` membership, then may still raise a
converter or backend error while constructing or solving the PySCIPOpt model.

This completes the Solver Adapter 🎉

```{note}
You can add parameter arguments in the inherited class in Python, so you can define additional parameters as follows. However, while this allows you to use various features of the backend solver, it may compromise compatibility with other adapters, so carefully consider when creating an adapter.

```python
    @classmethod
    def solve_strict(
        cls,
        ommx_instance: Instance,
        *,
        timeout: Optional[int] = None,
        diagnostics: DiagnosticsSink | None = None,
        **kwargs: Any,
    ) -> Solution:
```

### Solving a Knapsack Problem Using the Solver Adapter

For verification, let's solve a knapsack problem using this.

```{code-cell} ipython3
v = [10, 13, 18, 31, 7, 15]
w = [11, 25, 20, 35, 10, 33]
W = 47
N = len(v)

x = [
    DecisionVariable.binary(
        id=i,
        name="x",
        subscripts=[i],
    )
    for i in range(N)
]
instance = Instance.from_components(
    decision_variables=x,
    objective=sum(v[i] * x[i] for i in range(N)),
    constraints={0: sum(w[i] * x[i] for i in range(N)) - W <= 0},
    sense=Instance.MAXIMIZE,
)

solution = OMMXPySCIPOptAdapter.solve(instance)
```

## Implementing a Sampler Adapter

Next, let's create a Sampler Adapter using OpenJij. OpenJij includes [`openjij.SASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SASampler) for Simulated Annealing (SA) and [`openjij.SQASampler`](https://openjij.github.io/OpenJij/reference/openjij/index.html#openjij.SQASampler) for Simulated Quantum Annealing (SQA). In this tutorial, we will use `SASampler` as an example.

For simplicity, this tutorial omits the parameters passed to OpenJij. For more details, refer to the implementation of [`ommx-openjij-adapter`](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter). For how to use the OpenJij Adapter, refer to [Sampling from QUBO with OMMX Adapter](../tutorial/tsp_sampling_with_openjij_adapter).

### Converting `openjij.Response` to `ommx.Samples`

OpenJij manages decision variables with IDs that are not necessarily sequential, similar to OMMX, so there is no need to create an ID correspondence table as in the case of PySCIPOpt.

The sample results from OpenJij are obtained as `openjij.Response`, so implement a function to convert this to `ommx.Samples`. OpenJij returns the number of occurrences of the same sample as `num_occurrence`. On the other hand, `ommx.Samples` has unique sample IDs for each sample, and the same value samples are compressed as `SamplesEntry`. Note that a conversion is needed to bridge this difference.

```{code-cell} ipython3
import openjij as oj
from ommx import Instance, SampleSet, Solution, Samples, State

def decode_to_samples(response: oj.Response) -> Samples:
    # Generate sample IDs
    samples = Samples({})  # Create empty samples
    sample_id = 0

    num_reads = len(response.record.num_occurrences)
    for i in range(num_reads):
        sample = response.record.sample[i]
        state = State(entries=zip(response.variables, sample))
        # `num_occurrences` is encoded into sample ID list.
        # For example, if `num_occurrences` is 2, there are two samples with the same state, thus two sample IDs are generated.
        ids = []
        for _ in range(response.record.num_occurrences[i]):
            ids.append(sample_id)
            sample_id += 1
        samples.append(ids, state)

    return samples
```

Note that at this stage, `ommx.Instance` or its extracted correspondence table is not needed because there is no need to consider ID correspondence.

### Implementing a Class that Inherits `ommx.adapter.SamplerAdapter`

In the case of PySCIPOpt, we inherited `SolverAdapter`, but this time we will inherit `SamplerAdapter`. This has three `@abstractmethod` as follows:

```python
class SamplerAdapter(SolverAdapter):
    @classmethod
    @abstractmethod
    def sample_strict(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
        **kwargs: Any,
    ) -> SampleSet:
        pass

    @property
    @abstractmethod
    def sampler_input(self) -> SamplerInput:
        pass

    @abstractmethod
    def decode_to_sampleset(self, data: SamplerOutput) -> SampleSet:
        pass
```

`SamplerAdapter` supplies the same two-level API. The inherited `sample` copies
and prepares the caller's Instance, while `sample_strict` receives the exact
Adapter input. `SamplerAdapter.solve_strict` selects `best_feasible` from
`sample_strict`; `solver_input` delegates to `sampler_input`, and `decode`
returns `decode_to_sampleset(...).best_feasible`. A Sampler implementation
therefore defines only its sampler-owned `sample_strict`, `sampler_input`, and
`decode_to_sampleset` operations. The Easy and Solver bridges are inherited.

As with `solve`, the reserved `diagnostics` keyword is owned by `Run.log_sample`. A sampler may record adapter-defined reports into the sink when it is not `None`.

```{code-cell} ipython3
from typing import Any

from ommx.adapter import DiagnosticsSink, SamplerAdapter

class OMMXOpenJijSAAdapter(SamplerAdapter):
    """
    Sampling QUBO with Simulated Annealing (SA) by `openjij.SASampler`
    """

    INPUT_CLASS = InstanceClass(
        [
            InstanceClassClause(
                label="tutorial-binary-qubo",
                allowed_variable_kinds={Kind.Binary},
                objective_degree_bound=DegreeBound.at_most(2),
                allowed_senses={Sense.Minimize},
            )
        ]
    )

    # Retain the Instance because it is required to convert to SampleSet
    ommx_instance: Instance
    
    def __init__(self, ommx_instance: Instance):
        self.require_applicable(ommx_instance)
        self.ommx_instance = ommx_instance

    # Perform sampling
    def _sample(self) -> oj.Response:
        sampler = oj.SASampler()
        # Convert to QUBO dictionary format
        # QUBO conversion can fail here even after applicability was established.
        # This is a converter error, not an applicability result.
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return sampler.sample_qubo(qubo)

    # Common method for performing sampling
    @classmethod
    def sample_strict(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
        **kwargs: Any,
    ) -> SampleSet:
        _ = diagnostics
        adapter = cls(ommx_instance, **kwargs)
        response = adapter._sample()
        return adapter.decode_to_sampleset(response)
    
    # In this adapter, `SamplerInput` uses a QUBO dictionary
    @property
    def sampler_input(self) -> dict[tuple[int, int], float]:
        qubo, _offset = self.ommx_instance.as_qubo_format()
        return qubo
   
    # Convert OpenJij Response to a SampleSet
    def decode_to_sampleset(self, data: oj.Response) -> SampleSet:
        samples = decode_to_samples(data)
        # The information stored in `ommx.Instance` is required here
        return self.ommx_instance.evaluate_samples(samples)

```

### Sampling using our Adapter

Let's sample from the following QUBO using our Adapter:

$$
\begin{aligned}
\min & \quad -x_0 - x_1 + 2 x_0 x_1 \\
& \quad x_0, x_1 \in \{0, 1\}
\end{aligned}
$$

```{code-cell} ipython3
x = [DecisionVariable.binary(id, name="x", subscripts=[id]) for id in range(2)]
instance = Instance.from_components(
    decision_variables=x,
    objective=-x[0] - x[1] + 2 * x[0] * x[1],
    constraints={},
    sense=Instance.MINIMIZE,
)

sample_set = OMMXOpenJijSAAdapter.sample(instance)
sample_set.summary
```

## Summary

In this tutorial, we learned how to implement an OMMX Adapter by connecting to PySCIPOpt as a Solver Adapter and OpenJij as a Sampler Adapter. Here are the key points when implementing an OMMX Adapter:

1. Implement an OMMX Adapter by inheriting the abstract base class `SolverAdapter` or `SamplerAdapter`.
2. Define applicability with `INPUT_CLASS` and implement the preparation-free
   `solve_strict()` or `sample_strict()` method. The usual `solve()` and
   `sample()` APIs copy the caller's Instance and apply the fresh policy from
   `recommended_preparation_policy()` automatically. Applications customize
   Preparation by preparing an explicit copy and invoking the strict API.
3. The main steps of the implementation are as follows:
   - Convert `ommx.Instance` into a format that the backend solver can understand.
   - Run the backend solver to obtain a solution.
   - Convert the backend solver's output into `ommx.Solution` or `ommx.SampleSet`.
4. Keep converter-local and backend validation in the solver-input construction path, and report its failures as conversion or backend errors rather than applicability failures.
5. Pay attention to managing IDs and mapping variables to bridge the backend solver and OMMX.

If you want to connect your own backend solver to OMMX, refer to this tutorial for implementation. By implementing an OMMX Adapter following this tutorial, you can use optimization with various backend solvers through a common API.

For more detailed implementation examples, refer to the repositories such as [ommx-pyscipopt-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-pyscipopt-adapter) and [ommx-openjij-adapter](https://github.com/Jij-Inc/ommx/tree/main/python/ommx-openjij-adapter).
