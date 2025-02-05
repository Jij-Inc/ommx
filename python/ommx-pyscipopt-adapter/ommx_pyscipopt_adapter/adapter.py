from __future__ import annotations

import pyscipopt
import math

from ommx.adapter import SolverAdapter
from ommx.v1 import Instance, Solution, DecisionVariable, Constraint
from ommx.v1.function_pb2 import Function

from .exception import OMMXPySCIPOptAdapterError


class OMMXPySCIPOptAdapter(SolverAdapter):
    def __init__(self, ommx_instance: Instance):
        self.instance = ommx_instance
        self.model = pyscipopt.Model()
        self.model.hideOutput()

        self._set_decision_variables()
        self._set_objective()
        self._set_constraints()

    @staticmethod
    def solve(ommx_instance: Instance) -> Solution:
        pass

    @property
    def solver_input(self) -> pyscipopt.Model:
        pass

    def decode(self, data: pyscipopt.Model) -> Solution:
        pass

    def _set_decision_variables(self):
        for var in self.instance.decision_variables:
            if var.kind == DecisionVariable.BINARY:
                self.model.addVar(name=str(var.id), vtype="B")
            elif var.kind == DecisionVariable.INTEGER:
                self.model.addVar(
                    name=str(var.id), vtype="I", lb=var.bound.lower, ub=var.bound.upper
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self.model.addVar(
                    name=str(var.id), vtype="C", lb=var.bound.lower, ub=var.bound.upper
                )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Unsupported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )

        if self.instance.raw.objective.HasField("quadratic"):
            # If objective function is quadratic, add the auxiliary variable for the linealized objective function,
            # because the setObjective method in PySCIPOpt does not support quadratic objective functions.
            self.model.addVar(
                name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
            )

        self.varname_map = {var.name: var for var in self.model.getVars()}

    def _set_objective(self):
        objective = self.instance.raw.objective

        if self.instance.sense == Instance.MAXIMIZE:
            sense = "maximize"
        elif self.instance.sense == Instance.MINIMIZE:
            sense = "minimize"
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Sense not supported: {self.instance.sense}"
            )

        if objective.HasField("constant"):
            self.model.setObjective(objective.constant, sense=sense)
        elif objective.HasField("linear"):
            expr = self._make_linear_expr(objective)
            self.model.setObjective(expr, sense=sense)
        elif objective.HasField("quadratic"):
            # The setObjective method in PySCIPOpt does not support quadratic objective functions.
            # So we introduce the auxiliary variable to linearize the objective function,
            # Example:
            #     input problem: min x^2 + y^2
            #
            #     introduce the auxiliary variable z, and the linearized objective function problem is:
            #         min z
            #         s.t. z >= x^2 + y^2
            auxilary_var = self.varname_map["auxiliary_for_linearized_objective"]

            # Add the auxiliary variable to the objective function.
            self.model.setObjective(auxilary_var, sense=sense)

            # Add the constraint for the auxiliary variable.
            expr = self._make_quadratic_expr(objective)
            if sense == "minimize":
                constr_expr = auxilary_var >= expr
            else:  # sense == "maximize"
                constr_expr = auxilary_var <= expr

            self.model.addCons(constr_expr, name="constraint_for_linearized_objective")

        else:
            raise OMMXPySCIPOptAdapterError(
                "The objective function must be `constant`, `linear`, `quadratic`."
            )

        pass

    def _set_constraints(self):
        for constraint in self.instance.raw.constraints:
            if constraint.function.HasField("linear"):
                expr = self._make_linear_expr(constraint.function)
            elif constraint.function.HasField("quadratic"):
                expr = self._make_quadratic_expr(constraint.function)
            elif constraint.function.HasField("constant"):
                if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                    constraint.function.constant, 0, abs_tol=1e-6
                ):
                    continue
                elif (
                    constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                    and constraint.function.constant <= 1e-6
                ):
                    continue
                else:
                    raise OMMXPySCIPOptAdapterError(
                        f"Infeasible constant constraint was found: id {constraint.id}"
                    )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Constraints must be either `constant`, `linear` or `quadratic`."
                    f"id: {constraint.id}, "
                    f"type: {constraint.function.WhichOneof('function')}"
                )

            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                constr_expr = expr == 0
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = expr <= 0
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {constraint.id}, equality: {constraint.equality}"
                )

            self.model.addCons(constr_expr, name=str(constraint.id))

    def _make_linear_expr(self, function: Function) -> pyscipopt.Expr:
        linear = function.linear
        return (
            pyscipopt.quicksum(
                term.coefficient * self.varname_map[str(term.id)]
                for term in linear.terms
            )
            + linear.constant
        )

    def _make_quadratic_expr(self, function: Function) -> pyscipopt.Expr:
        quad = function.quadratic
        quad_terms = pyscipopt.quicksum(
            self.varname_map[str(row)] * self.varname_map[str(column)] * value
            for row, column, value in zip(quad.rows, quad.columns, quad.values)
        )

        linear_terms = pyscipopt.quicksum(
            term.coefficient * self.varname_map[str(term.id)]
            for term in quad.linear.terms
        )

        constant = quad.linear.constant

        return quad_terms + linear_terms + constant
