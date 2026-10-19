"""Persistent scaling guardrails for HiGHS model construction, without solving.

Origin: an interrupted conversion of a 255,565-term maintenance objective.
Purpose: detect quadratic copying when accumulating a long linear expression.
Boundary: OMMXHighsAdapter construction; OMMX fixture creation is excluded.
Independent variable: N active binary variables and N nonzero coefficients in
one objective or one constraint. All other dimensions remain fixed.
Cost model: O(N) variable creation plus O(N) expression construction; doubling
N should approximately double runtime, rather than quadruple it.
Input rationale: 1,000/2,000/4,000 terms expose growing-accumulator copies while
keeping three geometric scaling points and limiting instrumentation cost.
Lifecycle/run policy: run all six cases in CodSpeed CI to catch regressions in
model construction. The normal adapter test task excludes these benchmarks.
Run `task python:ommx-highs-adapter:bench` to measure them locally.
Runtime budget: six construction cases totaling 14,000 active variables,
with no optimization or external data.
"""

import pytest
from ommx import DecisionVariable, Instance, Linear, Sense

from ommx_highs_adapter import OMMXHighsAdapter


@pytest.fixture(params=[1_000, 2_000, 4_000])
def num_terms(request):
    return request.param


@pytest.fixture(params=["objective", "constraint"])
def conversion_instance(request, num_terms):
    variables = [DecisionVariable.binary(3 * i + 7) for i in range(num_terms)]
    expression = Linear(
        terms={var.id: float(1 + i % 7) for i, var in enumerate(variables)}, constant=3
    )
    return Instance.from_components(
        decision_variables=variables,
        objective=expression if request.param == "objective" else 0,
        constraints={0: expression <= num_terms}
        if request.param == "constraint"
        else {},
        sense=Sense.Minimize,
    )


@pytest.mark.benchmark_guardrail
@pytest.mark.benchmark
def test_model_construction(benchmark, conversion_instance):
    benchmark(OMMXHighsAdapter, conversion_instance)
