import highspy
import numpy as np

from highspy.highs import highs_linear_expression

from ommx.v1 import Instance, DecisionVariable, Solution, Constraint, State, Function
from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected

from .exception import OMMXHighsAdapterError


class OMMXHighsAdapter(SolverAdapter):
    """
    OMMX Adapter for HiGHS solver.

    This adapter translates OMMX optimization problems (ommx.v1.Instance) into HiGHS-compatible
    formats and converts HiGHS solutions back to OMMX format (ommx.v1.Solution).

    Translation Specifications
    ==========================

    Decision Variables
    ------------------
    The adapter handles the following translations for decision variables:

    **ID Management**:
    - OMMX: Variables managed by IDs (non-sequential integers)
    - HiGHS: Variables managed by array indices (0-based sequential)
    - Mapping maintained internally for bidirectional conversion

    **Variable Types**:

    | OMMX Type | HiGHS Type | Bounds |
    |-----------|------------|---------|
    | DecisionVariable.BINARY | HighsVarType.kInteger | [0, 1] |
    | DecisionVariable.INTEGER | HighsVarType.kInteger | [var.bound.lower, var.bound.upper] |
    | DecisionVariable.CONTINUOUS | HighsVarType.kContinuous | [var.bound.lower, var.bound.upper] |
    | DecisionVariable.SEMI_INTEGER | **Not supported** (support planned) | - |
    | DecisionVariable.SEMI_CONTINUOUS | **Not supported** (support planned) | - |

    **Note**: Semi-integer and semi-continuous variables are planned for future support but are
    currently unsupported. Using these variable types will raise an `OMMXHighsAdapterError`.

    Constraints
    -----------
    **Supported Function Types**:
    - Constant functions (ommx.v1.Function.constant)
    - Linear functions (ommx.v1.Function.linear)

    **Constraint Types**:

    | OMMX Constraint | Mathematical Form | HiGHS Constraint |
    |-----------------|-------------------|------------------|
    | Constraint.EQUAL_TO_ZERO | f(x) = 0 | const_expr == 0 |
    | Constraint.LESS_THAN_OR_EQUAL_TO_ZERO | f(x) ≤ 0 | const_expr <= 0 |

    **Constant Constraint Handling**:
    - Equality: Skip if |constant| ≤ 1e-10, error if |constant| > 1e-10
    - Inequality: Skip if constant ≤ 1e-10, error if constant > 1e-10

    **Constraint ID Management**:
    - OMMX constraint IDs converted to HiGHS constraint names via str(constraint.id)

    Objective Function
    ------------------
    **Optimization Direction**:

    | OMMX Direction | HiGHS Method |
    |----------------|--------------|
    | Instance.MINIMIZE | model.minimize(...) |
    | Instance.MAXIMIZE | model.maximize(...) |

    **Function Types**:
    - Constant objectives: Processing skipped
    - Linear objectives: Converted to HiGHS linear expressions

    Solution Decoding
    -----------------
    **Variable Values**: Extracted from HiGHS solution.col_value using maintained ID mapping
    **Optimality Status**: Set to OPTIMALITY_OPTIMAL when HiGHS returns kOptimal
    **Dual Variables**: Extracted from solution.row_dual for constraints

    Error Handling
    --------------
    **Unsupported Features**:
    - Quadratic functions (HiGHS supports linear problems only)
    - Semi-integer variables (DecisionVariable.SEMI_INTEGER, kind=4) - support planned
    - Semi-continuous variables (DecisionVariable.SEMI_CONTINUOUS, kind=5) - support planned
    - Constraint types other than EQUAL_TO_ZERO/LESS_THAN_OR_EQUAL_TO_ZERO

    **Solver Status Mapping**:

    | HiGHS Status | Exception |
    |--------------|-----------|
    | kInfeasible | InfeasibleDetected |
    | kUnbounded | UnboundedDetected |
    | kNotset | OMMXHighsAdapterError |

    Limitations
    -----------
    1. Linear problems only (no quadratic constraints or objectives)
    2. Constraint forms limited to equality (= 0) and inequality (≤ 0)
    3. Variable types limited to Binary, Integer, and Continuous
       - Semi-integer (SEMI_INTEGER) support is planned but not yet implemented
       - Semi-continuous (SEMI_CONTINUOUS) support is planned but not yet implemented

    Examples
    --------
    >>> from ommx_highs_adapter import OMMXHighsAdapter
    >>> from ommx.v1 import Instance, DecisionVariable
    >>>
    >>> # Define problem
    >>> x = DecisionVariable.binary(0)
    >>> y = DecisionVariable.integer(1, lower=0, upper=10)
    >>> instance = Instance.from_components(
    ...     decision_variables=[x, y],
    ...     objective=2*x + 3*y,
    ...     constraints=[x + y <= 5],
    ...     sense=Instance.MAXIMIZE,
    ... )
    >>>
    >>> # Solve
    >>> solution = OMMXHighsAdapter.solve(instance)
    >>> print(f"Optimal value: {solution.objective}")
    Optimal value: 15.0
    >>> print(f"Variables: {solution.state.entries}")
    Variables: {0: 0.0, 1: 5.0}

    """

    def __init__(self, ommx_instance: Instance, *, verbose: bool = False):
        """
        Initialize the adapter with an OMMX instance.

        Parameters
        ----------
        ommx_instance : Instance
            The OMMX optimization problem to solve
        verbose : bool, default=False
            If True, enable HiGHS's console logging
        """
        self.instance = ommx_instance
        self.model = highspy.Highs()

        # the default is for `log_to_console` to be True, so we
        # turn it off unless user requests it
        if not verbose:
            self.model.setOptionValue("log_to_console", False)

        self.var_ids = {}
        self.highs_vars = []

        self._set_decision_variables()
        self._set_objective()
        self._set_constraints()

    @classmethod
    def solve(cls, ommx_instance: Instance, *, verbose: bool = False) -> Solution:
        """
        Solve an OMMX optimization problem using HiGHS solver.

        This method provides a convenient interface for solving optimization problems
        without needing to manually instantiate the adapter. It handles the complete
        workflow: translation to HiGHS format, solving, and result conversion.

        Parameters
        ----------
        ommx_instance : Instance
            The OMMX optimization problem to solve. Must satisfy HiGHS adapter requirements:
            - Linear objective function (constant or linear terms only)
            - Linear constraints (constant or linear terms only)
            - Variables of type Binary, Integer, or Continuous only
              (Semi-integer and Semi-continuous support is planned but not yet implemented)
            - Constraints of type EQUAL_TO_ZERO or LESS_THAN_OR_EQUAL_TO_ZERO only

        verbose : bool, default=False
            If True, enable HiGHS's console logging for debugging

        Returns
        -------
        Solution
            The solution containing:
            - Variable values in solution.state.entries
            - Objective value in solution.objective
            - Constraint evaluations in solution.raw.evaluated_constraints
            - Optimality status in solution.raw.optimality
            - Dual variables (if available) in constraint.dual_variable

        Raises
        ------
        InfeasibleDetected
            When the optimization problem has no feasible solution
        UnboundedDetected
            When the optimization problem is unbounded
        OMMXHighsAdapterError
            When the problem contains unsupported features or HiGHS encounters an error

        Examples
        --------
        **Knapsack Problem**

        >>> from ommx.v1 import Instance, DecisionVariable, Solution
        >>> from ommx_highs_adapter import OMMXHighsAdapter
        >>>
        >>> p = [10, 13, 18, 32, 7, 15]  # profits
        >>> w = [11, 15, 20, 35, 10, 33]  # weights
        >>> x = [DecisionVariable.binary(i) for i in range(6)]
        >>> instance = Instance.from_components(
        ...     decision_variables=x,
        ...     objective=sum(p[i] * x[i] for i in range(6)),
        ...     constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>>
        >>> solution = OMMXHighsAdapter.solve(instance)
        >>> sorted([(id, value) for id, value in solution.state.entries.items()])
        [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
        >>> solution.feasible
        True
        >>> assert solution.optimality == Solution.OPTIMAL
        >>> solution.objective
        42.0

        **Infeasible Problem**

        >>> x = DecisionVariable.integer(0, upper=3, lower=0)
        >>> instance = Instance.from_components(
        ...     decision_variables=[x],
        ...     objective=x,
        ...     constraints=[x >= 4],  # Impossible: x ≤ 3 and x ≥ 4
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>> OMMXHighsAdapter.solve(instance)  # doctest: +IGNORE_EXCEPTION_DETAIL
        Traceback (most recent call last):
            ...
        ommx.adapter.InfeasibleDetected: Model was infeasible
        """
        # TODO would have added an unbounded example/doctest above,
        # but the same example used with pyscipopt isn't being correctly
        # detected as unbounded by HiGHS (simply returned x=0 as the solution)
        # requires further investigation
        #
        # the example for reference:
        # ```
        # >>> from ommx.v1 import Instance, DecisionVariable
        # >>> from ommx_highs_adapter import OMMXHighsAdapter

        # >>> x = DecisionVariable.integer(0, lower=0)
        # >>> instance = Instance.from_components(
        # ...     decision_variables=[x],
        # ...     objective=x,
        # ...     constraints=[],
        # ...     sense=Instance.MAXIMIZE,
        # ... )

        # >>> OMMXHighsAdapter.solve(instance)
        # Traceback (most recent call last):
        #     ...
        # ommx.adapter.UnboundedDetected: Model was unbounded
        # ````
        adapter = cls(ommx_instance, verbose=verbose)
        model = adapter.solver_input
        model.run()
        return adapter.decode(model)

    @property
    def solver_input(self) -> highspy.Highs:
        """
        The HiGHS model generated from the OMMX instance.

        Returns
        -------
        highspy.Highs
            The HiGHS model ready for optimization. This model contains:
            - Decision variables translated from OMMX IDs to HiGHS indices
            - Constraints converted to HiGHS linear expressions
            - Objective function set according to optimization direction
        """
        return self.model

    def decode(self, data: highspy.Highs) -> Solution:
        """
        Convert an optimized HiGHS model back to an OMMX Solution.

        This method translates HiGHS solver results into OMMX format, including
        variable values, optimality status, and dual variable information.

        Parameters
        ----------
        data : highspy.Highs
            The HiGHS model that has been optimized. Must be the same model
            returned by solver_input property.

        Returns
        -------
        Solution
            Complete OMMX solution containing:
            - Variable values mapped back to original OMMX IDs
            - Constraint evaluations and feasibility status
            - Optimality information from HiGHS
            - Dual variables for linear constraints

        Raises
        ------
        OMMXHighsAdapterError
            If the model has not been optimized yet
        InfeasibleDetected
            If HiGHS determined the problem is infeasible
        UnboundedDetected
            If HiGHS determined the problem is unbounded

        Notes
        -----
        This method should only be used after solving the model with HiGHS.
        Any modifications to the HiGHS model structure after creation may
        make the decoding process incompatible.

        The dual variables are extracted from HiGHS's row_dual and mapped
        to OMMX constraints based on their order. Only constraints with
        valid dual information will have the dual_variable field set.

        Examples
        --------
        >>> from ommx_highs_adapter import OMMXHighsAdapter
        >>> from ommx.v1 import Instance, DecisionVariable
        >>>
        >>> x = DecisionVariable.binary(0)
        >>> instance = Instance.from_components(
        ...     decision_variables=[x],
        ...     objective=x,
        ...     constraints=[],
        ...     sense=Instance.MAXIMIZE,
        ... )
        >>>
        >>> adapter = OMMXHighsAdapter(instance)
        >>> model = adapter.solver_input
        >>> model.run()  # doctest: +ELLIPSIS
        <...>
        >>> solution = adapter.decode(model)
        >>> solution.objective
        1.0
        """
        # TODO check if model is optimized
        state = self.decode_to_state(data)
        solution = self.instance.evaluate(state)

        # set optimality
        if self.model.getModelStatus() == highspy.HighsModelStatus.kOptimal:
            solution.raw.optimality = Solution.OPTIMAL

        # dual variables
        solution_info = self.model.getSolution()
        row_dual = solution_info.row_dual
        row_dual_len = len(row_dual)

        for constraint_id in solution.constraint_ids:
            if constraint_id < row_dual_len:
                solution.set_dual_variable(constraint_id, row_dual[constraint_id])

        return solution

    def decode_to_state(self, data: highspy.Highs) -> State:
        """
        Extract variable values from an optimized HiGHS model as an OMMX State.

        Parameters
        ----------
        data : highspy.Highs
            The optimized HiGHS model

        Returns
        -------
        State
            OMMX state containing variable values mapped to original OMMX IDs

        Raises
        ------
        OMMXHighsAdapterError
            If the model has not been optimized
        InfeasibleDetected
            If the model is infeasible
        UnboundedDetected
            If the model is unbounded

        Examples
        --------
        >>> from ommx_highs_adapter import OMMXHighsAdapter
        >>> from ommx.v1 import Instance, DecisionVariable
        >>>
        >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
        >>> instance = Instance.from_components(
        ...     decision_variables=[x1],
        ...     objective=x1,
        ...     constraints=[],
        ...     sense=Instance.MINIMIZE,
        ... )
        >>> adapter = OMMXHighsAdapter(instance)
        >>> model = adapter.solver_input
        >>> model.run()  # doctest: +ELLIPSIS
        <...>
        >>> state = adapter.decode_to_state(model)
        >>> state.entries
        {1: 0.0}
        """
        status = data.getModelStatus()
        if status == highspy.HighsModelStatus.kNotset:
            raise OMMXHighsAdapterError("Model has not been optimized")
        elif status == highspy.HighsModelStatus.kInfeasible:
            raise InfeasibleDetected("Model was infeasible")
        elif status == highspy.HighsModelStatus.kUnbounded:
            raise UnboundedDetected("Model was unbounded")

        solution = data.getSolution()
        return State(
            entries={
                var.id: solution.col_value[i]
                for i, var in enumerate(self.instance.used_decision_variables)
            }
        )

    def _set_decision_variables(self):
        num_cols = len(self.instance.used_decision_variables)
        lower = np.zeros(num_cols)
        upper = np.zeros(num_cols)
        types = []
        var_ids = []

        for i, var in enumerate(self.instance.used_decision_variables):
            var_ids.append(var.id)
            if var.kind == DecisionVariable.BINARY:
                lower[i] = 0
                upper[i] = 1
                types.append(highspy.HighsVarType.kInteger)
            elif var.kind == DecisionVariable.INTEGER:
                lower[i] = var.bound.lower
                upper[i] = var.bound.upper
                types.append(highspy.HighsVarType.kInteger)
            elif var.kind == DecisionVariable.CONTINUOUS:
                lower[i] = var.bound.lower
                upper[i] = var.bound.upper
                types.append(highspy.HighsVarType.kContinuous)
            else:
                raise OMMXHighsAdapterError(
                    f"Unsupported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )
        self.highs_vars = self.model.addVariables(
            var_ids, lb=lower.tolist(), ub=upper.tolist(), type=types
        )

    def _linear_expr_conversion(self, ommx_func: Function):
        # NOTE we explicityly don't convert to `highspy.highs.highs_linear_expression`
        # before returning as the callers want to check whether the returned
        # value is a constant float.
        if ommx_func.degree() >= 2:
            raise OMMXHighsAdapterError(
                "HiGHS Adapter currently only supports linear problems"
            )
        return (
            sum(
                coeff * self.highs_vars[id]
                for (id, coeff) in ommx_func.linear_terms.items()
            )
            + ommx_func.constant_term
        )

    def _set_objective(self):
        obj = self._linear_expr_conversion(self.instance.objective)
        if isinstance(obj, float):
            return
        if self.instance.sense == Instance.MAXIMIZE:
            self.model.maximize(highs_linear_expression(obj))
        elif self.instance.sense == Instance.MINIMIZE:
            self.model.minimize(highs_linear_expression(obj))
        else:
            raise OMMXHighsAdapterError(f"Unsupported sense: {self.instance.sense}")

    def _set_constraints(self):
        for constr in self.instance.constraints:
            const_expr = self._linear_expr_conversion(constr.function)
            if isinstance(const_expr, float):
                val = const_expr
                if constr.equality == Constraint.EQUAL_TO_ZERO:
                    if abs(val) > 1e-10:
                        raise OMMXHighsAdapterError(
                            "Infeasible constant equality constraint"
                        )
                    continue
                elif constr.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                    if val > 1e-10:
                        raise OMMXHighsAdapterError(
                            "Infeasible constant inequality constraint"
                        )
                    continue
            else:
                const_expr = highs_linear_expression(const_expr)
                if constr.equality == Constraint.EQUAL_TO_ZERO:
                    self.model.addConstr(const_expr == 0, str(constr.id))
                elif constr.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                    self.model.addConstr(const_expr <= 0, str(constr.id))
                else:
                    raise OMMXHighsAdapterError(
                        f"Unsupported constraint equality kind: {constr.equality}"
                    )
