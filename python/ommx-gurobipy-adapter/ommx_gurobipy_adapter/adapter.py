from __future__ import annotations
from typing import Literal

import gurobipy as gp
from gurobipy import GRB
import math

from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected
from ommx.v1 import Instance, Solution, DecisionVariable, Constraint
from ommx.v1.function_pb2 import Function
from ommx.v1.solution_pb2 import State, Optimality
from ommx.v1.constraint_hints_pb2 import ConstraintHints

from .exception import OMMXGurobipyAdapterError


HintMode = Literal["disabled", "auto", "forced"]


class OMMXGurobipyAdapter(SolverAdapter):
    use_sos1: HintMode

    def __init__(
        self,
        ommx_instance: Instance,
        *,
        use_sos1: Literal["disabled", "auto", "forced"] = "auto",
    ):
        self.instance = ommx_instance
        self.use_sos1 = use_sos1
        self.model = gp.Model()
        self.model.setParam("OutputFlag", 0)  # Suppress output

        self._set_decision_variables()
        self._set_objective()
        self._set_constraints()

    @staticmethod
    def solve(
        ommx_instance: Instance,
        *,
        use_sos1: Literal["disabled", "auto", "forced"] = "auto",
    ) -> Solution:
        """
        Solve the given ommx.v1.Instance using Gurobi, returning an ommx.v1.Solution.

        :param ommx_instance: The ommx.v1.Instance to solve.
        :param use_sos1: How to handle SOS1 constraints ("disabled", "auto", or "forced")
        :return: The solution as an ommx.v1.Solution object
        """
        adapter = OMMXGurobipyAdapter(ommx_instance, use_sos1=use_sos1)
        model = adapter.solver_input
        model.optimize()
        return adapter.decode(model)

    @property
    def solver_input(self) -> gp.Model:
        """The Gurobi model generated from this OMMX instance"""
        return self.model

    def decode(self, data: gp.Model) -> Solution:
        """Convert optimized Gurobi Model to ommx.v1.Solution."""

        status = data.Status

        if status == GRB.INFEASIBLE:
            raise InfeasibleDetected("Model was infeasible")

        if status == GRB.UNBOUNDED:
            raise UnboundedDetected("Model was unbounded")

        state = self.decode_to_state(data)
        solution = self.instance.evaluate(state)

        if status == GRB.OPTIMAL:
            solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL

        return solution

    def decode_to_state(self, data: gp.Model) -> State:
        """Create an ommx.v1.State from an optimized Gurobi Model."""

        if data.Status == GRB.LOADED:
            raise OMMXGurobipyAdapterError(
                "The model may not be optimized. [status: loaded]"
            )

        if data.Status == GRB.INFEASIBLE:
            raise InfeasibleDetected("Model was infeasible")

        if data.Status == GRB.UNBOUNDED:
            raise UnboundedDetected("Model was unbounded")

        try:
            if data.SolCount == 0:
                raise OMMXGurobipyAdapterError(
                    f"There is no feasible solution. [status: {data.Status}]"
                )

            return State(
                entries={
                    var.id: data.getVarByName(str(var.id)).X
                    for var in self.instance.raw.decision_variables
                }
            )
        except Exception as e:
            raise OMMXGurobipyAdapterError(f"Failed to decode solution: {str(e)}")

    def _set_decision_variables(self):
        """Set up decision variables in the Gurobi model."""
        for var in self.instance.raw.decision_variables:
            if var.kind == DecisionVariable.BINARY:
                self.model.addVar(name=str(var.id), vtype=GRB.BINARY)
            elif var.kind == DecisionVariable.INTEGER:
                self.model.addVar(
                    name=str(var.id),
                    vtype=GRB.INTEGER,
                    lb=var.bound.lower,
                    ub=var.bound.upper,
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self.model.addVar(
                    name=str(var.id),
                    vtype=GRB.CONTINUOUS,
                    lb=var.bound.lower,
                    ub=var.bound.upper,
                )
            else:
                raise OMMXGurobipyAdapterError(
                    f"Unsupported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )

        # Create map of OMMX variable IDs to Gurobi variables and ensure model is updated
        self.model.update()
        self.varname_map = {
            str(id): var
            for var, id in zip(
                self.model.getVars(),
                (var.id for var in self.instance.raw.decision_variables),
            )
        }

    def _set_objective(self):
        """Set up the objective function in the Gurobi model."""
        objective = self.instance.raw.objective

        # Set optimization direction
        if self.instance.sense == Instance.MAXIMIZE:
            self.model.ModelSense = GRB.MAXIMIZE
        elif self.instance.sense == Instance.MINIMIZE:
            self.model.ModelSense = GRB.MINIMIZE
        else:
            raise OMMXGurobipyAdapterError(
                f"Sense not supported: {self.instance.sense}"
            )

        # Set objective function
        if objective.HasField("constant"):
            self.model.setObjective(objective.constant)
        elif objective.HasField("linear"):
            expr = self._make_linear_expr(objective)
            self.model.setObjective(expr)
        elif objective.HasField("quadratic"):
            expr = self._make_quadratic_expr(objective)
            self.model.setObjective(expr)
        else:
            raise OMMXGurobipyAdapterError(
                "The objective function must be `constant`, `linear`, or `quadratic`."
            )

    def _set_constraints(self):
        """Set up constraints in the Gurobi model."""
        ommx_hints: ConstraintHints = self.instance.raw.constraint_hints
        excluded = set()

        # Handle SOS1 constraints
        if self.use_sos1 != "disabled":
            if self.use_sos1 == "forced" and len(ommx_hints.sos1_constraints) == 0:
                raise OMMXGurobipyAdapterError(
                    "No SOS1 constraints were found, but `use_sos1` is set to `forced`."
                )

            for sos1 in ommx_hints.sos1_constraints:
                bid = sos1.binary_constraint_id
                excluded.add(bid)
                name = f"sos1_{bid}"
                vars = [self.varname_map[str(v)] for v in sos1.decision_variables]
                self.model.addSOS(GRB.SOS_TYPE1, vars)

        # Handle regular constraints
        for constraint in self.instance.raw.constraints:
            if constraint.id in excluded:
                continue

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
                    raise OMMXGurobipyAdapterError(
                        f"Infeasible constant constraint was found: id {constraint.id}"
                    )
            else:
                raise OMMXGurobipyAdapterError(
                    f"Constraints must be either `constant`, `linear` or `quadratic`. "
                    f"id: {constraint.id}, "
                    f"type: {constraint.function.WhichOneof('function')}"
                )

            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                self.model.addConstr(expr == 0, name=str(constraint.id))
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                self.model.addConstr(expr <= 0, name=str(constraint.id))
            else:
                raise OMMXGurobipyAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {constraint.id}, equality: {constraint.equality}"
                )

    def _make_linear_expr(self, function: Function) -> gp.LinExpr:
        """Create a Gurobi linear expression from an OMMX Function."""
        linear = function.linear
        expr = gp.LinExpr()

        for term in linear.terms:
            var = self.varname_map[str(term.id)]
            expr.add(var, term.coefficient)

        expr.addConstant(linear.constant)
        return expr

    def _make_quadratic_expr(self, function: Function) -> gp.QuadExpr:
        """Create a Gurobi quadratic expression from an OMMX Function."""
        quad = function.quadratic
        expr = gp.QuadExpr()

        # Add quadratic terms
        for row, col, val in zip(quad.rows, quad.columns, quad.values):
            var1 = self.varname_map[str(row)]
            var2 = self.varname_map[str(col)]
            expr.add(var1 * var2 * val)

        # Add linear terms
        for term in quad.linear.terms:
            var = self.varname_map[str(term.id)]
            expr.add(var * term.coefficient)

        # Add constant
        expr.addConstant(quad.linear.constant)

        return expr
