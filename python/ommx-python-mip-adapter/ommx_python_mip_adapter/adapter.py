from __future__ import annotations

from typing import Optional

import mip

from ommx.adapter import SolverAdapter, InfeasibleDetected, UnboundedDetected
from ommx.v1 import Instance, Constraint, DecisionVariable, Solution, State, Optimality
from ommx.v1.function_pb2 import Function
from ommx.v1.solution_pb2 import Relaxation

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
        """
        :param ommx_instance: The ommx.v1.Instance to solve.
        :param relax: Applies integer relaxation globally to this model using Python-MIP's `Model.relax() <https://docs.python-mip.com/en/latest/classes.html#mip.Model.relax>`.
        :param solver_name: Passes a specific solver name to the Python-MIP model. Defaults to `CBC`.
        :param solver: Passes a specific solver to the Python-MIP model.
        :param verbose: If True, enable Python-MIP's verbose mode
        """
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

        self._set_decision_variables()
        self._set_objective()
        self._set_constraints()

        if relax:
            self.model.relax()
            self._relax = True
        else:
            self._relax = False

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        relax: bool = False,
        verbose: bool = False,
    ) -> Solution:
        """
        Solve the given ommx.v1.Instance using Python-MIP, returning an ommx.v1.Solution.

        :param ommx_instance: The ommx.v1.Instance to solve.
        :param relax: If True, relax all integer variables to continuous variables by using the `relax` parameter in Python-MIP's `Model.optimize() <https://docs.python-mip.com/en/latest/classes.html#mip.Model.optimize>`.
        :param verbose: If True, enable Python-MIP's verbose mode

        Examples
        =========

        KnapSack Problem

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx.v1.solution_pb2 import Optimality
            >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

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

            >>> solution = OMMXPythonMIPAdapter.solve(instance)

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
                >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

                >>> x = DecisionVariable.integer(0, upper=3, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints=[x >= 4],
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPythonMIPAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.InfeasibleDetected: Model was infeasible

        Unbounded Problem

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

                >>> x = DecisionVariable.integer(0, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints=[],
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPythonMIPAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.UnboundedDetected: Model was unbounded

        Dual variable

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

                >>> x = DecisionVariable.continuous(0, lower=0, upper=1)
                >>> y = DecisionVariable.continuous(1, lower=0, upper=1)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x, y],
                ...     objective=x + y,
                ...     constraints=[x + y <= 1],
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> solution = OMMXPythonMIPAdapter.solve(instance)
                >>> solution.raw.evaluated_constraints[0].dual_variable
                1.0

        """
        adapter = cls(ommx_instance, relax=relax, verbose=verbose)
        model = adapter.solver_input
        model.optimize(relax=relax)
        return adapter.decode(model)

    @property
    def solver_input(self) -> mip.Model:
        """The Python-MIP model generated from this OMMX instance"""
        return self.model

    def decode(self, data: mip.Model) -> Solution:
        """Convert optimized Python-MIP model and ommx.v1.Instance to ommx.v1.Solution.

        This method is intended to be used if the model has been acquired with
        `solver_input` for futher adjustment of the solver parameters, and
        separately optimizing the model.

        Note that alterations to the model may make the decoding process
        incompatible -- decoding will only work if the model still describes
        effectively the same problem as the OMMX instance used to create the
        adapter.

        When creating the solution, this method reflects the `relax` flag used
        in this adapter's constructor. The solution's `relaxation` metadata will
        be set _only_ if `relax=True` was passed to the constructor. There is no
        way for this adapter to get relaxation information from Python-MIP
        directly. If relaxing the model separately after obtaining it with
        `solver_input`, you must set `solution.raw.relaxation` yourself if you
        care about this value.

        Examples
        =========

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints=[sum(w[i] * x[i] for i in range(6)) <= 47],
            ...     sense=Instance.MAXIMIZE,
            ... )

            >>> adapter = OMMXPythonMIPAdapter(instance)
            >>> model = adapter.solver_input
            >>> # ... some modification of model's parameters
            >>> model.optimize()
            <OptimizationStatus.OPTIMAL: 0>

            >>> solution = adapter.decode(model)
            >>> solution.raw.objective
            42.0

        """
        # TODO check if `optimize()` has been called

        if data.status == mip.OptimizationStatus.INFEASIBLE:
            raise InfeasibleDetected("Model was infeasible")

        if data.status == mip.OptimizationStatus.UNBOUNDED:
            raise UnboundedDetected("Model was unbounded")

        state = self.decode_to_state(data)
        solution = self.instance.evaluate(state)

        dual_variables = {}
        for constraint in data.constrs:
            pi = constraint.pi
            if pi is not None:
                id = int(constraint.name)
                dual_variables[id] = pi
        for constraint in solution.raw.evaluated_constraints:
            id = constraint.id
            if id in dual_variables:
                constraint.dual_variable = dual_variables[id]

        if data.status == mip.OptimizationStatus.OPTIMAL:
            solution.raw.optimality = Optimality.OPTIMALITY_OPTIMAL

        if self._relax:
            solution.raw.relaxation = Relaxation.RELAXATION_LP_RELAXED
        return solution

    def decode_to_state(self, data: mip.Model) -> State:
        """
        Create an ommx.v1.State from an optimized Python-MIP Model.

        Examples
        =========

        .. doctest::

            The following example of solving an unconstrained linear optimization problem with x1 as the objective function.

            >>> from ommx_python_mip_adapter import OMMXPythonMIPAdapter
            >>> from ommx.v1 import Instance, DecisionVariable

            >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
            >>> ommx_instance = Instance.from_components(
            ...     decision_variables=[x1],
            ...     objective=x1,
            ...     constraints=[],
            ...     sense=Instance.MINIMIZE,
            ... )
            >>> adapter = OMMXPythonMIPAdapter(ommx_instance)
            >>> model = adapter.solver_input
            >>> model.optimize()
            <OptimizationStatus.OPTIMAL: 0>

            >>> ommx_state = adapter.decode_to_state(model)
            >>> ommx_state.entries
            {1: 0.0}
        """
        if not (
            data.status == mip.OptimizationStatus.OPTIMAL
            or data.status == mip.OptimizationStatus.FEASIBLE
        ):
            raise OMMXPythonMIPAdapterError(
                " The model's `status` must be `OPTIMAL` or `FEASIBLE`."
            )

        return State(
            entries={
                var.id: data.var_by_name(str(var.id)).x  # type: ignore
                for var in self.instance.raw.decision_variables
            }
        )

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
