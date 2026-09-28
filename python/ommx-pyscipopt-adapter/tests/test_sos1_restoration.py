from typing import Literal
import math

import pytest

from ommx import get_default_atol
from ommx.v1 import ConstraintHints, DecisionVariable, Instance, Sos1
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, OMMXPySCIPOptAdapterError

Mode = Literal["auto", "forced", "disabled"]


def formulation(*, case: str = "base", upper: float = 4, scale: float = 1) -> Instance:
    x = [DecisionVariable.continuous(i, lower=0, upper=upper) for i in (0, 1)]
    d = [DecisionVariable.binary(i) for i in (2, 3)]
    variables = x + d
    objective = x[0] + 2 * x[1]
    rows = [
        (scale * x[0] <= 4 * scale * d[0]).set_id(10),
        (scale * x[1] <= 4 * scale * d[1]).set_id(11),
        (d[0] + d[1] <= 1).set_id(12),
    ]
    hints = [
        Sos1(variables=[1, 0], binary_constraint_id=12, big_m_constraint_ids=[11, 10])
    ]
    if case == "objective":
        objective = objective - 100 * d[1]
    elif case == "retained_row":
        rows.append((d[1] == 0).set_id(13))
    elif case in ("shared_selector", "shared_member"):
        z = [DecisionVariable.continuous(i, lower=0, upper=4) for i in (4, 5)]
        variables += z
        selectors = d
        if case == "shared_member":
            z[0] = x[0]
            selectors = [DecisionVariable.binary(i) for i in (6, 7)]
            variables += selectors
        rows += [
            (z[0] <= 4 * selectors[0]).set_id(20),
            (z[1] <= 4 * selectors[1]).set_id(21),
            (selectors[0] + selectors[1] <= 1).set_id(22),
        ]
        objective = objective + 2 * z[0] + z[1]
        hints.append(
            Sos1(
                variables=[v.id for v in z],
                binary_constraint_id=22,
                big_m_constraint_ids=[20, 21],
            )
        )
    elif case == "overlap":
        hints.append(hints[0])
    elif case == "cardinality":
        rows[2] = (d[0] + d[1] == 1).set_id(12)
    elif case == "missing_link":
        rows[1] = (x[1] <= 4).set_id(11)
    elif case == "wrong_sign":
        rows[1] = (x[1] + 4 * d[1] <= 0).set_id(11)
    elif case == "extra_term":
        rows[1] = (x[1] - x[0] <= 4 * d[1]).set_id(11)
    elif case == "no_hints":
        hints = []
    elif case == "zero":
        objective = -x[0] - 2 * x[1]
    return Instance.from_components(
        decision_variables=variables,
        objective=objective,
        constraints=rows,
        sense=Instance.MAXIMIZE,
        constraint_hints=ConstraintHints(sos1_constraints=hints),
    )


@pytest.mark.parametrize("mode", ["auto", "forced", "disabled"])
def test_original_solution_is_returned_without_manual_repair(mode: Mode):
    instance = formulation()
    before = instance.to_bytes()
    adapter = OMMXPySCIPOptAdapter(instance, use_sos1=mode)
    model = adapter.solver_input
    assert {v.name for v in model.getVars()} == (
        {"0", "1", "2", "3"} if mode == "disabled" else {"0", "1"}
    )
    assert len(model.getConss()) == (3 if mode == "disabled" else 1)
    model.optimize()
    assert model.getStatus() == "optimal"
    assert model.getObjVal() == 8
    state = adapter.decode_to_state(model)
    assert state.entries == {0: 0, 1: 4, 2: 0, 3: 1}
    assert instance.evaluate(state).feasible
    for solution in (
        adapter.decode(model),
        OMMXPySCIPOptAdapter.solve(instance, use_sos1=mode),
    ):
        assert solution.feasible
        assert solution.objective == 8
        assert solution.optimality == solution.OPTIMAL
        assert all(solution.get_constraint_value(id) <= 0 for id in (10, 11, 12))
    assert instance.to_bytes() == before


@pytest.mark.parametrize("scale", [0.125, 1, 4])
def test_removed_links_tighten_only_the_solver_bounds(scale: float):
    instance = formulation(upper=100, scale=scale)
    before = instance.to_bytes()
    adapter = OMMXPySCIPOptAdapter(instance, use_sos1="forced")
    assert {v.name: v.getUbGlobal() for v in adapter.model.getVars()} == {
        "0": 4,
        "1": 4,
    }
    adapter.model.optimize()
    solution = adapter.decode(adapter.model)
    assert solution.feasible
    assert (
        solution.objective
        == OMMXPySCIPOptAdapter.solve(instance, use_sos1="disabled").objective
        == 8
    )
    assert instance.to_bytes() == before


@pytest.mark.parametrize(
    "case",
    [
        "objective",
        "retained_row",
        "shared_selector",
        "overlap",
        "cardinality",
        "missing_link",
        "wrong_sign",
        "extra_term",
    ],
)
def test_unsafe_hints_keep_original_rows_or_fail_explicitly(case: str):
    instance = formulation(case=case)
    before = instance.to_bytes()
    adapter = OMMXPySCIPOptAdapter(instance)
    assert all(c.getConshdlrName() != "SOS1" for c in adapter.model.getConss())
    assert {c.name for c in adapter.model.getConss()} == {
        str(c.id) for c in instance.constraints
    }
    adapter.model.optimize()
    solution = adapter.decode(adapter.model)
    assert solution.feasible
    assert (
        solution.objective
        == OMMXPySCIPOptAdapter.solve(instance, use_sos1="disabled").objective
    )
    with pytest.raises(OMMXPySCIPOptAdapterError, match="Cannot replace SOS1 hint"):
        OMMXPySCIPOptAdapter(instance, use_sos1="forced")
    assert instance.to_bytes() == before


def test_shared_members_with_private_selectors_can_be_promoted_together():
    instance = formulation(case="shared_member")
    adapter = OMMXPySCIPOptAdapter(instance, use_sos1="forced")
    assert len(adapter.model.getConss()) == 2
    assert not {"2", "3", "6", "7"} & {v.name for v in adapter.model.getVars()}
    adapter.model.optimize()
    solution = adapter.decode(adapter.model)
    assert solution.feasible
    assert (
        solution.objective
        == OMMXPySCIPOptAdapter.solve(instance, use_sos1="disabled").objective
    )


@pytest.mark.parametrize("integer", [False, True])
def test_negative_members_and_reused_binary_member(integer: bool):
    constructor = DecisionVariable.integer if integer else DecisionVariable.continuous
    x = constructor(42, lower=-100, upper=100)
    binary = DecisionVariable.binary(9)
    selector = DecisionVariable.binary(103)
    instance = Instance.from_components(
        decision_variables=[selector, x, binary],
        objective=-x,
        constraints=[
            (x <= 3 * selector).set_id(70),
            (-x <= 2.5 * selector).set_id(10),
            (selector + binary <= 1).set_id(20),
        ],
        sense=Instance.MAXIMIZE,
        constraint_hints=ConstraintHints(
            sos1_constraints=[
                Sos1(
                    variables=[42, 9],
                    binary_constraint_id=20,
                    big_m_constraint_ids=[70, 10],
                )
            ]
        ),
    )
    adapter = OMMXPySCIPOptAdapter(instance, use_sos1="forced")
    assert {v.name for v in adapter.model.getVars()} == {"42", "9"}
    adapter.model.optimize()
    solution = adapter.decode(adapter.model)
    assert solution.feasible
    assert solution.objective == (2 if integer else 2.5)
    assert solution.state.entries[103] == 1
    assert (
        solution.objective
        == OMMXPySCIPOptAdapter.solve(instance, use_sos1="disabled").objective
    )


def test_all_zero_and_initial_solution():
    instance = formulation(case="zero")
    solution = OMMXPySCIPOptAdapter.solve(
        instance, use_sos1="forced", initial_state={0: 0, 1: 0, 2: 1, 3: 0}
    )
    assert solution.feasible
    assert solution.objective == 0
    assert solution.state.entries == {0: 0, 1: 0, 2: 0, 3: 0}


def test_near_zero_restoration_uses_original_link_residuals():
    instance = formulation()
    adapter = OMMXPySCIPOptAdapter(instance, use_sos1="forced")
    tiny = math.nextafter(get_default_atol(), 0.0)
    entries = {0: tiny, 1: 4.0}
    adapter._sos1_plans[0].restore(entries)
    assert instance.evaluate(entries).feasible
    assert entries[2] == 0 and entries[3] == 1

    # V2 excludes equality at the tolerance boundary.
    with pytest.raises(OMMXPySCIPOptAdapterError, match="feasibility tolerance"):
        adapter._sos1_plans[0].restore({0: get_default_atol(), 1: 4.0})

    # The same member value is not negligible after scaling the original row.
    scaled = OMMXPySCIPOptAdapter(formulation(scale=4), use_sos1="forced")
    with pytest.raises(OMMXPySCIPOptAdapterError, match="feasibility tolerance"):
        scaled._sos1_plans[0].restore({0: tiny, 1: 4.0})


@pytest.mark.parametrize("mode", ["auto", "forced", "disabled"])
def test_scaled_links_restore_feasible_solver_solution(mode: Mode):
    value = 2 * get_default_atol()
    x = DecisionVariable.continuous(0, lower=value, upper=value)
    y = DecisionVariable.continuous(1, lower=0, upper=4)
    p, q = [DecisionVariable.binary(i) for i in (2, 3)]
    instance = Instance.from_components(
        decision_variables=[x, y, p, q],
        objective=y,
        constraints=[
            (0.125 * x <= 0.5 * p).set_id(10),
            (0.125 * y <= 0.5 * q).set_id(11),
            (p + q <= 1).set_id(12),
        ],
        sense=Instance.MAXIMIZE,
        constraint_hints=ConstraintHints(
            sos1_constraints=[
                Sos1(
                    variables=[0, 1],
                    binary_constraint_id=12,
                    big_m_constraint_ids=[10, 11],
                )
            ]
        ),
    )
    expected = {0: value, 1: 4, 2: 0, 3: 1}
    # The member exceeds ATol, but its original link residual at p=0 does not.
    assert instance.evaluate(expected).feasible
    adapter = OMMXPySCIPOptAdapter(instance, use_sos1=mode)
    adapter.model.setRealParam("numerics/feastol", 10 * get_default_atol())
    adapter.model.setIntParam("presolving/maxrounds", 0)
    adapter.model.optimize()
    assert adapter.model.getStatus() == "optimal"
    assert adapter.decode_to_state(adapter.model).entries == expected
    solution = adapter.decode(adapter.model)
    assert solution.feasible
    assert solution.objective == 4


def test_no_hints_preserves_mode_contract():
    instance = formulation(case="no_hints")
    assert OMMXPySCIPOptAdapter.solve(instance).feasible
    with pytest.raises(OMMXPySCIPOptAdapterError, match="No SOS1 constraints"):
        OMMXPySCIPOptAdapter(instance, use_sos1="forced")


@pytest.mark.parametrize(
    "lower,upper,selector_lower,selector_upper,scale,big_m,reason",
    [
        (-1, 4, 0, 1, 1, 4, "missing its lower link"),
        (5, 10, 0, 1, 1, 4, "empty or unbounded"),
        (0, 4, 1, 1, 1, 4, "bound excludes"),
        (0, 4, 0, 0, 1, 4, "bound excludes"),
        # 4/3 cannot be represented exactly as a SCIP variable bound. Rounding
        # outward must not make an insufficient link appear to cover it.
        (0, 4, 0, 1, 3, 4, "does not cover"),
    ],
)
def test_incompatible_domains_and_links_are_not_promoted(
    lower: float,
    upper: float,
    selector_lower: float,
    selector_upper: float,
    scale: float,
    big_m: float,
    reason: str,
):
    x = DecisionVariable.continuous(0, lower=lower, upper=upper)
    selector = DecisionVariable.of_type(
        DecisionVariable.BINARY,
        1,
        lower=selector_lower,
        upper=selector_upper,
    )
    instance = Instance.from_components(
        decision_variables=[x, selector],
        objective=x,
        constraints=[
            (scale * x <= big_m * selector).set_id(10),
            (selector <= 1).set_id(11),
        ],
        sense=Instance.MAXIMIZE,
        constraint_hints=ConstraintHints(
            sos1_constraints=[
                Sos1(variables=[0], binary_constraint_id=11, big_m_constraint_ids=[10])
            ]
        ),
    )
    with pytest.raises(OMMXPySCIPOptAdapterError, match=reason):
        OMMXPySCIPOptAdapter(instance, use_sos1="forced")
    adapter = OMMXPySCIPOptAdapter(instance)
    assert len(adapter.model.getConss()) == 2
    assert {v.name for v in adapter.model.getVars()} == {"0", "1"}
