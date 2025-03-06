import highspy
import numpy as np

from highspy.highs import highs_linear_expression

from ommx.v1 import Instance, DecisionVariable, Solution, Constraint
from ommx.v1.solution_pb2 import State, Optimality
from ommx.v1.function_pb2 import Function
from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected

from .exception import OMMXHighsAdapterError


class OMMXHighsAdapter(SolverAdapter):
    def __init__(self, ommx_instance: Instance, *, verbose: bool = False):
        """
        :param verbose: If True, enable HiGHS's console logging
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
        Solve the given ommx.v1.Instance using HiGHS, returning an ommx.v1.Solution.

        :param verbose: If True, enable HiGHS's console logging

        Examples
        =========

        Knapsack Problem

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx.v1.solution_pb2 import Optimality
            >>> from ommx_highs_adapter import OMMXHighsAdapter

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
            ...     sense=Instance.MAXIMIZE,
            ... )

            Solve it

            >>> solution = OMMXHighsAdapter.solve(instance)

            Check output

            >>> sorted([(id, value) for id, value in solution.state.entries.items()])
            [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
            >>> solution.feasible
            True
            >>> assert solution.optimality == Optimality.OPTIMALITY_OPTIMAL

            p[0] + p[3] = 42
            w[0] + w[3] = 46 <= 47

            >>> solution.objective
            42.0
            >>> solution.raw.evaluated_constraints[0].evaluated_value
            -1.0

        Infeasible Problem

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_highs_adapter import OMMXHighsAdapter

                >>> x = DecisionVariable.integer(0, upper=3, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints=[x >= 4],
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXHighsAdapter.solve(instance)
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
        """The HiGHS model generated from this OMMX instance"""
        return self.model

    def decode(self, data: highspy.Highs) -> Solution:
        """Convert an optimized highspy.HiGHS based on this instance into an ommx.v1.Solution .

        This method is intended to be used if the model has been acquired with
        `solver_input` for futher adjustment of the solver parameters, and
        separately optimizing the model.

        Note that alterations to the model may make the decoding process
        incompatible -- decoding will only work if the model still describes
        effectively the same problem as the OMMX instance used to create the
        adapter.

        Examples
        =========

        .. doctest::

            >>> from ommx_highs_adapter import OMMXHighsAdapter
            >>> from ommx.v1 import Instance, DecisionVariable

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
            ...     sense=Instance.MAXIMIZE,
            ... )

            >>> adapter = OMMXHighsAdapter(instance)
            >>> model = adapter.solver_input
            >>> # ... some modification of model's parameters
            >>> model.run()
            <HighsStatus.kOk: 0>

            >>> solution = adapter.decode(model)
            >>> solution.raw.objective
            42.0
        """
        # TODO check if model is optimized
        state = self.decode_to_state(data)
        solution = self.instance.evaluate(state)

        # set optimality
        if self.model.getModelStatus() == highspy.HighsModelStatus.kOptimal:
            solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL

        # dual variables
        solution_info = self.model.getSolution()
        for constraint in solution.raw.evaluated_constraints:
            if constraint.id < len(solution_info.row_dual):
                constraint.dual_variable = solution_info.row_dual[constraint.id]

        return solution

    def decode_to_state(self, data: highspy.Highs) -> State:
        """
        Create an ommx.v1.State from an optimized HiGHS Model.

        Examples
        =========

        .. doctest::

            The following example shows how to solve an unconstrained linear optimization problem with `x1` as the objective function.

            >>> from ommx_highs_adapter import OMMXHighsAdapter
            >>> from ommx.v1 import Instance, DecisionVariable

            >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
            >>> ommx_instance = Instance.from_components(
            ...     decision_variables=[x1],
            ...     objective=x1,
            ...     constraints=[],
            ...     sense=Instance.MINIMIZE,
            ... )
            >>> adapter = OMMXHighsAdapter(ommx_instance)
            >>> model = adapter.solver_input
            >>> model.run()
            <HighsStatus.kOk: 0>
            >>> ommx_state = adapter.decode_to_state(model)
            >>> ommx_state.entries
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
                for i, var in enumerate(self.instance.raw.decision_variables)
            }
        )

    def _set_decision_variables(self):
        num_cols = len(self.instance.raw.decision_variables)
        lower = np.zeros(num_cols)
        upper = np.zeros(num_cols)
        types = []
        var_ids = []

        for i, var in enumerate(self.instance.raw.decision_variables):
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
        if ommx_func.HasField("constant"):
            return ommx_func.constant
        elif ommx_func.HasField("linear"):
            return (
                sum(
                    term.coefficient * self.highs_vars[term.id]
                    for term in ommx_func.linear.terms
                )
                + ommx_func.linear.constant
            )

        else:
            raise OMMXHighsAdapterError(
                "The function must be either `constant` or `linear`."
            )

    def _set_objective(self):
        obj = self._linear_expr_conversion(self.instance.raw.objective)
        if isinstance(obj, float):
            return
        if self.instance.raw.sense == Instance.MAXIMIZE:
            self.model.maximize(highs_linear_expression(obj))
        elif self.instance.raw.sense == Instance.MINIMIZE:
            self.model.minimize(highs_linear_expression(obj))
        else:
            raise OMMXHighsAdapterError(f"Unsupported sense: {self.instance.raw.sense}")

    def _set_constraints(self):
        for constr in self.instance.raw.constraints:
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
