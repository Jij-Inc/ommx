from __future__ import annotations

from typing import Optional
from dataclasses import dataclass

import mip

from ommx.adapter import SolverAdapter
from ommx.v1 import Instance, Constraint, DecisionVariable, Solution, State, Optimality
from ommx.v1.function_pb2 import Function
from ommx.v1.solution_pb2 import Result, Infeasible, Unbounded, Relaxation

from .exception import OMMXPythonMIPAdapterError


class OMMXPythonMIPAdapter(SolverAdapter):
    def __init__(
        self,
        ommx_instance: Instance,
        *,
        relax: bool = False,
        solver_name: str = mip.CBC,
        solver: Optional[mip.Solver] = None,
        verbose: bool = False,
    ):
        if ommx_instance.raw.sense == Instance.MAXIMIZE:
            sense = mip.MAXIMIZE
        elif ommx_instance.raw.sense == Instance.MINIMIZE:
            sense = mip.MINIMIZE
        else:
            raise OMMXPythonMIPAdapterError(
                f"Unsupported sense: {ommx_instance.raw.sense}"
            )
        self.instance = ommx_instance
        self.model = mip.Model(
            sense=sense,
            solver_name=solver_name,
            solver=solver,
        )
        if verbose:
            self.model.verbose = 1
        else:
            self.model.verbose = 0

        self.set_decision_variables()
        self.set_objective()
        self.set_constraints()

        if relax:
            self.model.relax()
            self.relax = True


    @staticmethod
    def solve(
        ommx_instance: Instance,
        relax: bool = False,
        verbose: bool = False,
    ) -> Solution:
        adapter = OMMXPythonMIPAdapter(ommx_instance, relax=relax, verbose=verbose)
        model = adapter.solver_input
        model.optimize(relax=relax)
        return adapter.decode(model)


    @property
    def solver_input(self) -> mip.Model:
        pass

    def decode(self, model: mip.Model) -> Solution:
        # TODO check if `optimize()` has been called

        if model.status == mip.OptimizationStatus.INFEASIBLE:
            return Result(infeasible=Infeasible())

        if model.status == mip.OptimizationStatus.UNBOUNDED:
            return Result(unbounded=Unbounded())
        state = State(
            entries={
                var.id: model.var_by_name(str(var.id)).x  # type: ignore
                for var in self.instance.raw.decision_variables
            }
        )

        solution = self.instance.evaluate(state)

        dual_variables = {}
        for constraint in model.constrs:
            pi = constraint.pi
            if pi is not None:
                id = int(constraint.name)
                dual_variables[id] = pi
        for constraint in solution.raw.evaluated_constraints:
            id = constraint.id
            if id in dual_variables:
                constraint.dual_variable = dual_variables[id]

        if model.status == mip.OptimizationStatus.OPTIMAL:
            solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL

            if self.relax:
                solution.raw.relaxation = Relaxation.RELAXATION_LP_RELAXED
        return solution

    def relax(self):
        """
        Enables relaxation of integer to continuous in the Python-MIP model.

        This is not reversible.
        """
        self.relax = True
        self.model.relax()


    def _set_decision_variables(self):
        for var in self.instance.raw.decision_variables:
            if var.kind == DecisionVariable.BINARY:
                self.model.add_var(
                    name=str(var.id),
                    var_type=mip.BINARY,
                )
            elif var.kind == DecisionVariable.INTEGER:
                self.model.add_var(
                    name=str(var.id),
                    var_type=mip.INTEGER,
                    lb=var.bound.lower,  # type: ignore
                    ub=var.bound.upper,  # type: ignore
                )
            elif var.kind == DecisionVariable.CONTINUOUS:
                self.model.add_var(
                    name=str(var.id),
                    var_type=mip.CONTINUOUS,
                    lb=var.bound.lower,  # type: ignore
                    ub=var.bound.upper,  # type: ignore
                )
            else:
                raise OMMXPythonMIPAdapterError(
                    f"Not supported decision variable kind: "
                    f"id: {var.id}, kind: {var.kind}"
                )

    def _as_lin_expr(
        self,
        f: Function,
    ) -> mip.LinExpr:
        """
        Translate ommx.v1.Function to `mip.LinExpr` or `float`.
        """
        if f.HasField("constant"):
            return mip.LinExpr(const=f.constant)  # type: ignore
        elif f.HasField("linear"):
            ommx_linear = f.linear
            return (
                mip.xsum(
                    term.coefficient * self.model.vars[str(term.id)]  # type: ignore
                    for term in ommx_linear.terms
                )
                + ommx_linear.constant
            )  # type: ignore
        raise OMMXPythonMIPAdapterError(
            "The function must be either `constant` or `linear`."
        )

    def _set_objective(self):
        self.model.objective = self._as_lin_expr(self.instance.raw.objective)  # type: ignore

    def _set_constraints(self):
        for constraint in self.instance.raw.constraints:
            lin_expr = self._as_lin_expr(constraint.function)
            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                constr_expr = lin_expr == 0
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = lin_expr <= 0  # type: ignore
            else:
                raise OMMXPythonMIPAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {constraint.id}, equality: {constraint.equality}"
                )
            self.model.add_constr(constr_expr, name=str(constraint.id))
