from __future__ import annotations
from dataclasses import dataclass
from typing import ClassVar, Optional

import pyscipopt
import math

from opentelemetry import trace

from ommx.adapter import (
    DiagnosticEntry,
    DiagnosticsSink,
    JsonObject,
    SolverAdapter,
    InfeasibleDetected,
    UnboundedDetected,
    NoSolutionReturned,
)
from ommx.v1 import (
    Instance,
    Solution,
    DecisionVariable,
    Function,
    Constraint,
    AdditionalCapability,
    State,
    ToState,
)

from .exception import OMMXPySCIPOptAdapterError

_tracer = trace.get_tracer("ommx.adapter.pyscipopt")

_SCIP_TERMINATION_REPORT_SCHEMA = "org.ommx.solver.scip.termination-report.v1"
_SCIP_TERMINATION_REPORT_NAME = "solver/scip/termination-report"
_SCIP_TERMINATION_REPORT_KIND = "termination_report"
_SCIP_SOLVER_NAME = "scip"
_ADAPTER_NAME = "ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter"


def _finite_float_or_none(value: float) -> float | None:
    value = float(value)
    if math.isfinite(value):
        return value
    return None


@dataclass(frozen=True, slots=True)
class SCIPTerminationReport:
    """Post-solve termination summary produced by SCIP."""

    SCHEMA: ClassVar[str] = _SCIP_TERMINATION_REPORT_SCHEMA
    NAME: ClassVar[str] = _SCIP_TERMINATION_REPORT_NAME
    KIND: ClassVar[str] = _SCIP_TERMINATION_REPORT_KIND

    status: str
    primal_bound: float | None
    dual_bound: float | None
    gap: float | None
    objective_value: float | None
    node_count: int
    solution_count: int
    solving_time_sec: float | None
    scip_version: str
    pyscipopt_version: str | None

    @classmethod
    def from_model(cls, model: pyscipopt.Model) -> SCIPTerminationReport:
        solution_count = int(model.getNSols())
        return cls(
            status=str(model.getStatus()),
            primal_bound=_finite_float_or_none(model.getPrimalbound()),
            dual_bound=_finite_float_or_none(model.getDualbound()),
            gap=_finite_float_or_none(model.getGap()),
            objective_value=(
                _finite_float_or_none(model.getObjVal()) if solution_count > 0 else None
            ),
            node_count=int(model.getNNodes()),
            solution_count=solution_count,
            solving_time_sec=_finite_float_or_none(model.getSolvingTime()),
            scip_version=(
                f"{model.getMajorVersion()}.{model.getMinorVersion()}.{model.getTechVersion()}"
            ),
            pyscipopt_version=getattr(pyscipopt, "__version__", None),
        )

    def to_json(self) -> JsonObject:
        return {
            "adapter": _ADAPTER_NAME,
            "dual_bound": self.dual_bound,
            "gap": self.gap,
            "node_count": self.node_count,
            "objective_value": self.objective_value,
            "primal_bound": self.primal_bound,
            "pyscipopt_version": self.pyscipopt_version,
            "schema": self.SCHEMA,
            "scip_version": self.scip_version,
            "solution_count": self.solution_count,
            "solver": _SCIP_SOLVER_NAME,
            "solving_time_sec": self.solving_time_sec,
            "status": self.status,
        }

    @classmethod
    def from_json(cls, data: JsonObject) -> SCIPTerminationReport:
        schema = _required_str(data, "schema")
        if schema != cls.SCHEMA:
            msg = (
                f"SCIP termination report schema is {schema!r}, expected {cls.SCHEMA!r}"
            )
            raise ValueError(msg)
        return cls(
            status=_required_str(data, "status"),
            primal_bound=_optional_float(data, "primal_bound"),
            dual_bound=_optional_float(data, "dual_bound"),
            gap=_optional_float(data, "gap"),
            objective_value=_optional_float(data, "objective_value"),
            node_count=_required_int(data, "node_count"),
            solution_count=_required_int(data, "solution_count"),
            solving_time_sec=_optional_float(data, "solving_time_sec"),
            scip_version=_required_str(data, "scip_version"),
            pyscipopt_version=_optional_str(data, "pyscipopt_version"),
        )

    def to_entry(self) -> DiagnosticEntry:
        return DiagnosticEntry.from_json_diagnostic(
            self,
            annotations={"org.ommx.solver.name": _SCIP_SOLVER_NAME},
        )


def _record_scip_termination_report(
    model: pyscipopt.Model, diagnostics: DiagnosticsSink
) -> None:
    diagnostics.record(SCIPTerminationReport.from_model(model))


def _required_str(data: JsonObject, key: str) -> str:
    value = data.get(key)
    if not isinstance(value, str):
        msg = f"SCIP termination report `{key}` must be a string"
        raise TypeError(msg)
    return value


def _optional_str(data: JsonObject, key: str) -> str | None:
    value = data.get(key)
    if value is None:
        return None
    if not isinstance(value, str):
        msg = f"SCIP termination report `{key}` must be a string or null"
        raise TypeError(msg)
    return value


def _required_int(data: JsonObject, key: str) -> int:
    value = data.get(key)
    if isinstance(value, bool) or not isinstance(value, int):
        msg = f"SCIP termination report `{key}` must be an integer"
        raise TypeError(msg)
    return value


def _optional_float(data: JsonObject, key: str) -> float | None:
    value = data.get(key)
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int | float):
        msg = f"SCIP termination report `{key}` must be a number or null"
        raise TypeError(msg)
    return float(value)


class OMMXPySCIPOptAdapter(SolverAdapter):
    SUPPORTS_DIAGNOSTICS = True
    ADDITIONAL_CAPABILITIES = frozenset(
        {
            AdditionalCapability.Indicator,
            AdditionalCapability.Sos1,
        }
    )

    def __init__(
        self,
        ommx_instance: Instance,
        *,
        initial_state: Optional[ToState] = None,
    ):
        """
        :param ommx_instance: The ommx.v1.Instance to solve.
        :param initial_state: Optional initial solution state.
        """
        with _tracer.start_as_current_span("convert"):
            super().__init__(ommx_instance)
            self.instance = ommx_instance
            self.model = pyscipopt.Model()
            self.model.hideOutput()

            self._set_decision_variables()
            self._set_objective()
            self._set_constraints()

            # Add initial solution if provided
            if initial_state is not None:
                self._add_initial_state(initial_state)

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        initial_state: Optional[ToState] = None,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """
        Solve the given ommx.v1.Instance using PySCIPopt, returning an ommx.v1.Solution.

        :param ommx_instance: The ommx.v1.Instance to solve.
        :param initial_state: Optional initial solution state.

        Examples
        =========

        KnapSack Problem

        .. doctest::

            >>> from ommx.v1 import Instance, DecisionVariable
            >>> from ommx.v1 import Solution
            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
            ...     sense=Instance.MAXIMIZE,
            ... )

            Solve it

            >>> solution = OMMXPySCIPOptAdapter.solve(instance)

            Check output

            >>> sorted([(id, value) for id, value in solution.state.entries.items()])
            [(0, 1.0), (1, 0.0), (2, 0.0), (3, 1.0), (4, 0.0), (5, 0.0)]
            >>> solution.feasible
            True
            >>> assert solution.optimality == Solution.OPTIMAL

            p[0] + p[3] = 42
            w[0] + w[3] = 46 <= 47

            >>> solution.objective
            42.0
            >>> solution.get_constraint_value(0)
            -1.0

        Infeasible Problem

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

                >>> x = DecisionVariable.integer(0, upper=3, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints={0: x >= 4},
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPySCIPOptAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.InfeasibleDetected: Model was infeasible

        Unbounded Problem

        .. doctest::

                >>> from ommx.v1 import Instance, DecisionVariable
                >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

                >>> x = DecisionVariable.integer(0, lower=0)
                >>> instance = Instance.from_components(
                ...     decision_variables=[x],
                ...     objective=x,
                ...     constraints={},
                ...     sense=Instance.MAXIMIZE,
                ... )

                >>> OMMXPySCIPOptAdapter.solve(instance)
                Traceback (most recent call last):
                    ...
                ommx.adapter.UnboundedDetected: Model was unbounded
        """
        with _tracer.start_as_current_span("solve") as span:
            span.set_attribute("adapter", f"{cls.__module__}.{cls.__qualname__}")
            adapter = cls(ommx_instance, initial_state=initial_state)
            model = adapter.solver_input
            with _tracer.start_as_current_span("call"):
                model.optimize()
            solution = adapter.decode(model)
            if diagnostics is not None:
                _record_scip_termination_report(model, diagnostics)
            return solution

    @property
    def solver_input(self) -> pyscipopt.Model:
        """The PySCIPOpt model generated from this OMMX instance"""
        return self.model

    def decode(self, data: pyscipopt.Model) -> Solution:
        """Convert optimized pyscipopt.Model and ommx.v1.Instance to ommx.v1.Solution.

        This method is intended to be used if the model has been acquired with
        `solver_input` for further adjustment of the solver parameters, and
        separately optimizing the model.

        Note that alterations to the model may make the decoding process
        incompatible -- decoding will only work if the model still describes
        effectively the same problem as the OMMX instance used to create the
        adapter.

        Examples
        =========

        .. doctest::

            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
            >>> from ommx.v1 import Instance, DecisionVariable

            >>> p = [10, 13, 18, 32, 7, 15]
            >>> w = [11, 15, 20, 35, 10, 33]
            >>> x = [DecisionVariable.binary(i) for i in range(6)]
            >>> instance = Instance.from_components(
            ...     decision_variables=x,
            ...     objective=sum(p[i] * x[i] for i in range(6)),
            ...     constraints={0: sum(w[i] * x[i] for i in range(6)) <= 47},
            ...     sense=Instance.MAXIMIZE,
            ... )

            >>> adapter = OMMXPySCIPOptAdapter(instance)
            >>> model = adapter.solver_input
            >>> # ... some modification of model's parameters
            >>> model.optimize()

            >>> solution = adapter.decode(model)
            >>> solution.objective
            42.0

        """

        # TODO: Add the feature to store dual variables in `solution`.

        with _tracer.start_as_current_span("decode"):
            state = self.decode_to_state(data)
            solution = self.instance.evaluate(state)

            if (
                data.getStatus() == "optimal"
            ):  # pyscipopt does not appear to have an enum or constant for this
                solution.optimality = Solution.OPTIMAL

            return solution

    def decode_to_state(self, data: pyscipopt.Model) -> State:
        """
        Create an ommx.v1.State from an optimized PySCIPOpt Model.

        Examples
        =========

        .. doctest::

            The following example shows how to solve an unconstrained linear optimization problem with `x1` as the objective function.

            >>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
            >>> from ommx.v1 import Instance, DecisionVariable

            >>> x1 = DecisionVariable.integer(1, lower=0, upper=5)
            >>> ommx_instance = Instance.from_components(
            ...     decision_variables=[x1],
            ...     objective=x1,
            ...     constraints={},
            ...     sense=Instance.MINIMIZE,
            ... )
            >>> adapter = OMMXPySCIPOptAdapter(ommx_instance)
            >>> model = adapter.solver_input
            >>> model.optimize()

            >>> ommx_state = adapter.decode_to_state(model)
            >>> ommx_state.entries
            {1: 0.0}

        """
        if data.getStatus() == "unknown":
            raise OMMXPySCIPOptAdapterError(
                "The model may not be optimized. [status: unknown]"
            )

        if data.getStatus() == "infeasible":
            raise InfeasibleDetected("Model was infeasible")

        if data.getStatus() == "unbounded":
            raise UnboundedDetected("Model was unbounded")

        if data.getStatus() == "timelimit":
            # The following condition checks if there is no feasible primal solution.
            # In other words, it is checking for the absence of any feasible solution.
            if data.getNSols() == 0:
                raise NoSolutionReturned("No solution was returned [status: timelimit]")

        # NOTE: It is assumed that getBestSol will return an error
        #       if there is no feasible solution.
        try:
            sol = data.getBestSol()
            # NOTE recreating the map instead of using `self.varname_map`, as
            # this is probably more robust.
            varname_map = {var.name: var for var in data.getVars()}
            return State(
                entries={
                    var.id: sol[varname_map[str(var.id)]]
                    for var in self.instance.used_decision_variables
                }
            )
        except Exception:
            raise OMMXPySCIPOptAdapterError(
                f"There is no feasible solution. [status: {data.getStatus()}]"
            )

    def _set_decision_variables(self):
        for var in self.instance.used_decision_variables:
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

        # Check if objective is quadratic to add auxiliary variable
        degree = self.instance.objective.degree()
        if degree > 3:
            raise OMMXPySCIPOptAdapterError(
                f"Objective function degree {degree} is not supported. "
                "Only constant, linear, and quadratic objectives are supported."
            )
        if degree == 2:
            # If objective function is quadratic, add the auxiliary variable for the linearized objective function,
            # because the setObjective method in PySCIPOpt does not support quadratic objective functions.
            self.model.addVar(
                name="auxiliary_for_linearized_objective", vtype="C", lb=None, ub=None
            )

        self.varname_map = {var.name: var for var in self.model.getVars()}

    def _set_objective(self):
        if self.instance.sense == Instance.MAXIMIZE:
            sense = "maximize"
        elif self.instance.sense == Instance.MINIMIZE:
            sense = "minimize"
        else:
            raise OMMXPySCIPOptAdapterError(
                f"Sense not supported: {self.instance.sense}"
            )

        objective = self.instance.objective

        degree = objective.degree()
        if degree == 0:
            self.model.setObjective(objective.constant_term, sense=sense)
        elif degree == 1:
            expr = self._make_linear_expr(objective)
            self.model.setObjective(expr, sense=sense)
        elif degree == 2:
            # The setObjective method in PySCIPOpt does not support quadratic objective functions.
            # So we introduce the auxiliary variable to linearize the objective function,
            # Example:
            #     input problem: min x^2 + y^2
            #
            #     introduce the auxiliary variable z, and the linearized objective function problem is:
            #         min z
            #         s.t. z >= x^2 + y^2
            auxiliary_var = self.varname_map["auxiliary_for_linearized_objective"]

            # Add the auxiliary variable to the objective function.
            self.model.setObjective(auxiliary_var, sense=sense)

            # Add the constraint for the auxiliary variable.
            expr = self._make_quadratic_expr(objective)
            if sense == "minimize":
                constr_expr = auxiliary_var >= expr
            else:  # sense == "maximize"
                constr_expr = auxiliary_var <= expr

            self.model.addCons(constr_expr, name="constraint_for_linearized_objective")
        else:
            raise OMMXPySCIPOptAdapterError(
                "The objective function must be constant, linear, or quadratic."
            )

    def _set_constraints(self):
        # Handle SOS1 constraints (first-class constraint type)
        for sos1_id, sos1 in self.instance.sos1_constraints.items():
            name = f"sos1_{sos1_id}"
            vars = [self.varname_map[str(v)] for v in sos1.variables]
            self.model.addConsSOS1(vars, name=name)

        for cid, constraint in self.instance.constraints.items():
            # Handle constraint function based on its type
            f = constraint.function
            degree = f.degree()
            if degree == 0:
                # Constant constraint is not passed to SCIP, but checked for feasibility
                constant_value = f.constant_term
                if constraint.equality == Constraint.EQUAL_TO_ZERO and math.isclose(
                    constant_value, 0, abs_tol=1e-6
                ):
                    continue  # Skip feasible constant constraint
                elif (
                    constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                    and constant_value <= 1e-6
                ):
                    continue  # Skip feasible constant constraint
                else:
                    raise OMMXPySCIPOptAdapterError(
                        f"Infeasible constant constraint was found: id {cid}"
                    )
            elif degree == 1:
                expr = self._make_linear_expr(f)
            elif degree == 2:
                expr = self._make_quadratic_expr(f)
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Constraints must be either constant, linear or quadratic. "
                    f"id: {cid}, "
                    f"degree: {degree}"
                )

            if constraint.equality == Constraint.EQUAL_TO_ZERO:
                constr_expr = expr == 0
            elif constraint.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                constr_expr = expr <= 0
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Not supported constraint equality: "
                    f"id: {cid}, equality: {constraint.equality}"
                )

            self.model.addCons(constr_expr, name=str(cid))

        # Handle indicator constraints
        for ind_id, indicator in self.instance.indicator_constraints.items():
            f = indicator.function
            degree = f.degree()
            if degree == 0:
                # Constant indicator constraint: check feasibility statically
                # When indicator is ON, the constant constraint must hold
                constant_value = f.constant_term
                is_feasible = (
                    indicator.equality == Constraint.EQUAL_TO_ZERO
                    and math.isclose(constant_value, 0, abs_tol=1e-6)
                ) or (
                    indicator.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
                    and constant_value <= 1e-6
                )
                if is_feasible:
                    continue  # Always feasible, skip
                # If infeasible when indicator is ON, add indicator constraint
                # that forces indicator to be 0
                binvar = self.varname_map[str(indicator.indicator_variable_id)]
                self.model.addCons(binvar == 0, name=f"ind_{ind_id}_forced_off")
                continue
            elif degree == 1:
                expr = self._make_linear_expr(f)
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Indicator constraints must be linear. "
                    f"id: {ind_id}, degree: {degree}"
                )

            binvar = self.varname_map[str(indicator.indicator_variable_id)]

            if indicator.equality == Constraint.EQUAL_TO_ZERO:
                # Decompose f(x) == 0 into two indicator constraints
                self.model.addConsIndicator(
                    expr <= 0, binvar=binvar, name=f"ind_{ind_id}_le"
                )
                self.model.addConsIndicator(
                    -expr <= 0, binvar=binvar, name=f"ind_{ind_id}_ge"
                )
            elif indicator.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO:
                self.model.addConsIndicator(
                    expr <= 0, binvar=binvar, name=f"ind_{ind_id}"
                )
            else:
                raise OMMXPySCIPOptAdapterError(
                    f"Not supported indicator constraint equality: "
                    f"id: {ind_id}, equality: {indicator.equality}"
                )

    def _make_linear_expr(self, f: Function) -> pyscipopt.Expr:
        return (
            pyscipopt.quicksum(
                coeff * self.varname_map[str(id)]
                for id, coeff in f.linear_terms.items()
            )
            + f.constant_term
        )

    def _make_quadratic_expr(self, f: Function) -> pyscipopt.Expr:
        # Quadratic terms
        quad_terms = pyscipopt.quicksum(
            self.varname_map[str(row)] * self.varname_map[str(col)] * coeff
            for (row, col), coeff in f.quadratic_terms.items()
        )

        # Linear terms
        linear_terms = pyscipopt.quicksum(
            coeff * self.varname_map[str(var_id)]
            for var_id, coeff in f.linear_terms.items()
        )

        constant = f.constant_term

        return quad_terms + linear_terms + constant

    def _add_initial_state(self, initial_state: ToState) -> None:
        initial_sol = self.model.createSol()
        for var_id, value in State(initial_state).entries.items():
            var_name = str(var_id)
            if var_name in self.varname_map:
                self.model.setSolVal(initial_sol, self.varname_map[var_name], value)
        # The free=True parameter means that solution will be freed afterwards.
        self.model.addSol(initial_sol, free=True)
