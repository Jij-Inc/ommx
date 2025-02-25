import highspy
import numpy as np

from ommx.v1 import Instance, DecisionVariable, Solution, Constraint
from ommx.v1.solution_pb2 import State, Optimality
from ommx.v1.function_pb2 import Function
from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected

from .exception import OMMXHighsAdapterError


class OMMXHighsAdapter(SolverAdapter):
    def __init__(self, ommx_instance: Instance, *, verbose: bool = False):
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

    @staticmethod
    def solve(ommx_instance: Instance, *, verbose: bool = False) -> Solution:
        """
        Solve the given ommx.v1.Instance using HiGHS, returning an ommx.v1.Solution.

        Examples:
            >>> from ommx_highs_adapter import OMMXHighsAdapter
            >>> from ommx.v1 import Instance, DecisionVariable
            >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
            >>> instance = Instance.from_components(
            ...     decision_variables=[x1],
            ...     objective=x1,
            ...     constraints=[],
            ...     sense=Instance.MINIMIZE
            ... )
            >>> solution = OMMXHighsAdapter.solve(instance)
            >>> solution.objective
            0.0
        """
        adapter = OMMXHighsAdapter(ommx_instance, verbose=verbose)
        model = adapter.solver_input
        model.run()
        return adapter.decode(model)

    @property
    def solver_input(self) -> highspy.Highs:
        return self.model

    def decode(self, data: highspy.Highs) -> Solution:
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
            self.model.maximize(obj)
        elif self.instance.raw.sense == Instance.MINIMIZE:
            self.model.minimize(obj)
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
                if constr.equality == Constraint.EQUAL_TO_ZERO:
                    self.model.addConstr(const_expr == 0, str(constr.id))
                elif constr.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                    self.model.addConstr(const_expr <= 0, str(constr.id))
                else:
                    raise OMMXHighsAdapterError(
                        f"Unsupported constraint equality kind: {constr.equality}"
                    )
