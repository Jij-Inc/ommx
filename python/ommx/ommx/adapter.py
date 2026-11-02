"""Common interfaces for solving and sampling an OMMX Instance.

.. rubric:: Compare results from multiple adapters

Use the same :class:`~ommx.Instance` with different adapters to compare their
returned :class:`~ommx.Solution` objects. This complete example needs the SDK,
``ommx-highs-adapter``, ``ommx-pyscipopt-adapter``, pandas, and matplotlib; follow
the :doc:`installation guide </getting_started/install>` to install compatible
versions. No external data or previous example state is needed.

.. rubric:: Build one problem and solve it twice

Choose items to maximize their total value without exceeding a weight capacity
of 47. Each binary variable is 1 when the corresponding item is selected. The
variable name ``x`` and its subscript ``i`` retain that correspondence for result
extraction later. OMMX also assigns each variable an ID within the Instance;
names and subscripts are labels, not those IDs. The Instance holds the variables,
objective expression, and capacity constraint together.

>>> from ommx import Instance
>>> values = [10, 13, 18, 31, 7, 15]
>>> weights = [11, 25, 20, 35, 10, 33]
>>> instance = Instance.maximize()
>>> x = [instance.new_binary("x", subscripts=[i]) for i in range(len(values))]
>>> instance.objective = sum(value * var for value, var in zip(values, x))
>>> _ = instance.add_constraint(
...     sum(weight * var for weight, var in zip(weights, x)) <= 47,
...     "weight capacity",
... )

Both HiGHS and SCIP can directly accept this linear binary model. Each adapter's
``solve`` prepares an isolated copy, so the same source Instance can be reused.
Other models can require different preparation for each backend. See
:class:`~ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter` for explicit policy selection
and :doc:`Adapter packages and input contracts </api/adapters>` for other backends.

>>> from ommx_highs_adapter import OMMXHighsAdapter
>>> from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
>>> adapters = {"highs": OMMXHighsAdapter, "scip": OMMXPySCIPOptAdapter}
>>> solutions = {name: adapter.solve(instance) for name, adapter in adapters.items()}

.. rubric:: Compare reported values and guarantees

The reported objective is OMMX's evaluated value; the optimization direction
(``sense``) tells you whether larger or smaller is better. Feasibility includes
variable domains and constraints. A feasible candidate, matching objective
values, and a proven optimum are separate observations. In this small example,
both adapters return an optimal feasible solution with objective 41.

>>> import pandas as pd
>>> comparison = pd.DataFrame([
...     {
...         "adapter": name,
...         "sense": repr(solution.sense),
...         "objective": solution.objective,
...         "feasible": solution.feasible,
...         "optimality": repr(solution.optimality),
...     }
...     for name, solution in solutions.items()
... ]).set_index("adapter")
>>> print(comparison.to_string())  # doctest: +NORMALIZE_WHITESPACE
                  sense  objective  feasible          optimality
adapter
highs    Sense.Maximize       41.0      True  Optimality.Optimal
scip     Sense.Maximize       41.0      True  Optimality.Optimal

This demonstrates a comparison procedure, not which solver is generally better.
Align the following conditions before drawing conclusions:

.. list-table::
   :header-rows: 1

   * - Information
     - Conditions for comparison
   * - Reported objective and sense
     - Same problem and objective meaning.
   * - Feasibility for the original problem
     - Same evaluated conditions and numerical tolerances.
   * - Optimality
     - Check the problem and guarantee represented by each adapter's result.
   * - Backend bounds, gaps, and termination reasons
     - Check the solved formulation, solver-facing objective, and backend definitions.
   * - Runtime
     - Measure separately with defined data, settings, environment, and repetitions.

If you explicitly solve a transformed Instance, evaluate the candidate against
the original Instance as needed. Reconstructed values and membership in an
adapter's input class alone do not establish original feasibility or global
optimality. See :doc:`Transform problems and choose a solving method
</user_guide/transform_and_solve>` and :doc:`Evaluate candidates and interpret
results </user_guide/evaluate_and_interpret>` for choosing the evaluation target.

The :class:`~ommx_highs_adapter.HighsDiagnosticsAnalyzer` and
:class:`~ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer` references explain backend
reports. A bound on a sign-changed or penalized objective is not automatically a
bound on the original objective. :class:`~ommx.experiment.Experiment` can organize
repeated runs, but is not required for this comparison.

.. rubric:: Plot and tabulate the selected items

:meth:`~ommx.Solution.extract_decision_variables` selects variables named ``x``
and returns their values keyed by subscript tuples. Here ``(i,)`` corresponds to
item ``i``; sort these keys to put the items in model order. Variables without
the requested name are omitted. Duplicate subscript tuples under that name are
an error rather than silently overwriting a value; use distinct labels or inspect
the ID-indexed DataFrame when labels are not unique.

>>> selected = solutions["highs"].extract_decision_variables("x")
>>> [selected[(i,)] for i in range(len(values))]
[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
>>> from matplotlib import pyplot as plt
>>> fig, ax = plt.subplots()
>>> marks = {"highs": "o", "scip": "+"}
>>> for name, solution in solutions.items():
...     selected = solution.extract_decision_variables("x")
...     keys = sorted(selected)
...     _ = ax.plot(
...         [key[0] for key in keys], [selected[key] for key in keys],
...         marks[name], label=name,
...     )
>>> _ = ax.set(xlabel="Item", ylabel="Selected (1) or not selected (0)")
>>> _ = ax.legend()
>>> len(ax.lines)
2

Display ``fig`` in a notebook or call ``plt.show()`` interactively. Close the
figure when finished to release its resources.

>>> plt.close(fig)

For tabular analysis, :meth:`~ommx.Solution.decision_variables_df` provides one
row per variable. Add a solver column before concatenating the tables so the
origin of each row remains explicit; variable IDs can repeat across solutions.

>>> decision_variables = pd.concat([
...     solution.decision_variables_df().assign(solver=name)
...     for name, solution in solutions.items()
... ])
>>> decision_variables.groupby("solver").size().to_dict()
{'highs': 6, 'scip': 6}
"""

from __future__ import annotations

import copy
from abc import ABC, abstractmethod
from typing import Any, ClassVar, Protocol, runtime_checkable

from ommx._ommx_rust import DiagnosticCollector as DiagnosticCollector
from ommx import (
    InfeasibleDetected as InfeasibleDetected,
    Instance,
    InstanceClass,
    InstanceClassMembershipReport,
    PreparationPolicy,
    SampleSet,
    Solution,
)


SolverInput = Any
SolverOutput = Any
SamplerInput = Any
SamplerOutput = Any


@runtime_checkable
class DiagnosticReport(Protocol):
    """Adapter diagnostic report convertible with ``dataclasses.asdict``."""

    __dataclass_fields__: ClassVar[dict[str, Any]]


class DiagnosticsSink(Protocol):
    """Receiver for adapter-defined diagnostics emitted during a solve.

    Adapters may call ``record`` while the backend solver is still running,
    including from backend callbacks. Sink implementations should keep
    ``record`` append-only, defer validation or serialization until after the
    solve, and preserve the order in which diagnostics are received.

    A conforming sink must not raise from ``record``. If recording fails, the
    sink should log the failure and return normally. If ``record`` does raise,
    that is a sink contract violation; adapters may let the exception propagate
    and do not need to recover from it.
    """

    def record(self, diagnostic: DiagnosticReport) -> None:
        """Record one adapter-defined dataclass diagnostic report or event.

        This method must not raise under normal sink failures. Custom sinks
        should log failures and return instead.
        """


class AdapterNotApplicableError(ValueError):
    """Raised when an instance is not applicable to an adapter."""

    adapter: str
    report: InstanceClassMembershipReport

    def __init__(self, adapter: str, report: InstanceClassMembershipReport):
        self.adapter = adapter
        self.report = report
        super().__init__(f"{adapter} is not applicable:\n{report}")


class SolverAdapter(ABC):
    """
    An abstract interface for OMMX Solver Adapters, defining how solvers should be used with OMMX.

    See the `Adapter implementation tutorial <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_
    for the complete implementation contract and reference implementation.

    Subclasses declare ``INPUT_CLASS`` as the OMMX-defined structural class of
    exact inputs they can encode. Membership in that class is the complete
    applicability condition: ``check_applicability`` only checks membership and
    does not mutate the input. A subclass should call ``require_applicable``
    before constructing backend input.

    Membership describes the exact input, not every problem that could become
    acceptable after transformation. It does not run preparation or a backend,
    guarantee that a candidate will be returned, or rule out converter/backend
    errors such as implementation limits or numerical failures. Those errors
    are distinct from the membership-based applicability decision.

    The easy :meth:`solve` API prepares an isolated copy with the Adapter's
    recommended policy. Use :meth:`solve_without_preparation` when the caller
    has prepared an exact input explicitly and wants the Adapter to require
    ``INPUT_CLASS`` membership without applying preparation.
    Adapter constructors likewise require an already applicable input.
    The :class:`ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter` Reference includes
    a complete custom-policy example from model construction to strict solve.

    When encoding OMMX decision variables, use
    ``ommx_instance.used_decision_variables`` rather than the complete decision
    variable table. Backend-only auxiliary variables may be added separately.
    Preserve the correspondence between each used OMMX variable ID and its
    backend representation. Decoding must recover a value for every used OMMX
    variable, construct a :class:`ommx.State`, and evaluate it against an
    unchanged :class:`ommx.Instance` snapshot that is mathematically identical
    to the exact input encoded for the backend. The Instance owns population of
    fixed, dependent, and irrelevant variables and construction of the final
    :class:`ommx.Solution`.
    """

    INPUT_CLASS: ClassVar[InstanceClass]
    """Required condition for an exact Adapter input."""

    @classmethod
    def recommended_preparation_policy(cls) -> PreparationPolicy:
        """Return a fresh policy recommended for this Adapter's ``INPUT_CLASS``.

        The easy APIs apply it to an isolated copy. Advanced callers may edit
        and apply it explicitly before using a preparation-free API. This
        method itself neither prepares an instance nor guarantees
        applicability. It does not inspect any particular Instance, and the
        class and policy are independent values. Application-specific choices
        such as penalty weights may still need to be supplied by the caller.
        The default policy is empty.
        """
        return PreparationPolicy()

    @classmethod
    def check_applicability(
        cls, ommx_instance: Instance
    ) -> InstanceClassMembershipReport:
        """Check ``INPUT_CLASS`` membership without mutation."""
        input_class: InstanceClass | None = getattr(cls, "INPUT_CLASS", None)
        if input_class is None:
            raise TypeError(
                f"{cls.__module__}.{cls.__qualname__} must declare INPUT_CLASS"
            )

        return input_class.check_membership(ommx_instance)

    @classmethod
    def require_applicable(
        cls, ommx_instance: Instance
    ) -> InstanceClassMembershipReport:
        """Return the membership report or raise ``AdapterNotApplicableError``."""
        report = cls.check_applicability(ommx_instance)
        if not report.is_member:
            adapter = f"{cls.__module__}.{cls.__qualname__}"
            raise AdapterNotApplicableError(adapter, report)
        return report

    @classmethod
    def solve(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Prepare and solve an isolated copy of an OMMX instance.

        The input ``ommx_instance`` is never modified. The copy is prepared for
        ``INPUT_CLASS`` with :meth:`recommended_preparation_policy`, then passed
        to :meth:`solve_without_preparation`.

        ``Run.log_solve`` owns the reserved ``diagnostics`` keyword. When
        called with ``store_diagnostics=True``, it passes a sink to the adapter
        and stores recorded diagnostics with the Solve entry. Adapters may
        record adapter-defined dataclass diagnostics into the sink during the
        solve; ``None`` means diagnostics are disabled. Adapters do not need to
        catch exceptions raised by a non-conforming diagnostics sink.
        """
        prepared = copy.copy(ommx_instance)
        prepared.prepare(cls.INPUT_CLASS, cls.recommended_preparation_policy())
        return cls.solve_without_preparation(prepared, diagnostics=diagnostics)

    @classmethod
    @abstractmethod
    def solve_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Solve an exact Adapter input without running ``Instance.prepare``.

        ``ommx_instance`` must belong to ``INPUT_CLASS``. Implementations must
        reject non-members with :class:`AdapterNotApplicableError` and must not
        prepare or otherwise modify the input instance.
        """
        pass

    @property
    @abstractmethod
    def solver_input(self) -> SolverInput:
        pass

    @abstractmethod
    def decode(self, data: SolverOutput) -> Solution:
        pass


class SamplerAdapter(SolverAdapter):
    """
    An abstract interface for OMMX Sampler Adapters, defining how samplers should be used with OMMX.

    See the `Adapter implementation tutorial <https://jij-inc-ommx.readthedocs-hosted.com/en/latest/tutorial/implement_adapter.html>`_
    for the complete implementation contract and reference implementations.

    The exact-input, easy-API preparation, strict preparation-free API, and
    used-variable contracts of :class:`SolverAdapter` also apply to sampler
    adapters. A sampler whose
    backend labels are the OMMX variable IDs may preserve that identity directly;
    other representations need an explicit correspondence. In either case, each
    decoded sample must contain a value for every variable in
    ``ommx_instance.used_decision_variables`` before ``evaluate_samples`` is
    called against an unchanged :class:`ommx.Instance` snapshot mathematically
    identical to the encoded exact input. Backend formats that omit a
    zero-interaction variable therefore need an adapter-owned decoding default
    consistent with that format.
    """

    @classmethod
    def sample(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        """Prepare and sample an isolated copy of an OMMX instance.

        The input ``ommx_instance`` is never modified. The copy is prepared for
        ``INPUT_CLASS`` with :meth:`recommended_preparation_policy`, then passed
        to :meth:`sample_without_preparation`.

        ``Run.log_sample`` owns the reserved ``diagnostics`` keyword and uses
        it the same way as ``Run.log_solve``. ``None`` means diagnostics are
        disabled.
        """
        prepared = copy.copy(ommx_instance)
        prepared.prepare(cls.INPUT_CLASS, cls.recommended_preparation_policy())
        return cls.sample_without_preparation(prepared, diagnostics=diagnostics)

    @classmethod
    @abstractmethod
    def sample_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> SampleSet:
        """Sample an exact Adapter input without running ``Instance.prepare``.

        ``ommx_instance`` must belong to ``INPUT_CLASS``. Implementations must
        reject non-members with :class:`AdapterNotApplicableError` and must not
        prepare or otherwise modify the input instance.
        """
        pass

    @classmethod
    def solve_without_preparation(
        cls,
        ommx_instance: Instance,
        *,
        diagnostics: DiagnosticsSink | None = None,
    ) -> Solution:
        """Return the best feasible result from :meth:`sample_without_preparation`."""
        return cls.sample_without_preparation(
            ommx_instance,
            diagnostics=diagnostics,
        ).best_feasible

    @property
    def solver_input(self) -> SolverInput:
        """Expose :attr:`sampler_input` through the SolverAdapter interface."""
        return self.sampler_input

    def decode(self, data: SolverOutput) -> Solution:
        """Decode sampler output and return its best feasible solution."""
        return self.decode_to_sampleset(data).best_feasible

    @property
    @abstractmethod
    def sampler_input(self) -> SamplerInput:
        pass

    @abstractmethod
    def decode_to_sampleset(self, data: SamplerOutput) -> SampleSet:
        pass


class UnboundedDetected(Exception):
    """
    Raised when the problem is proven to be unbounded.

    This corresponds to ``Optimality.OPTIMALITY_UNBOUNDED`` and indicates that
    the mathematical model itself is unbounded.
    Should not be used when unboundedness cannot be proven (e.g., heuristic solvers).
    """

    pass


class NoSolutionReturned(Exception):
    """
    Raised when no solution was returned.

    This indicates that the solver did not return any solution (whether feasible
    or not) (e.g., due to time limits).
    This does not prove that the mathematical model itself is infeasible.
    """

    pass
