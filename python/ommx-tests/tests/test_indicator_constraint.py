"""Tests for IndicatorConstraint Big-M conversion to regular constraints."""

import pandas as pd
import pytest
from ommx.v1 import (
    Instance,
    DecisionVariable,
    IndicatorConstraint,
    Equality,
)


def _df_snap(df: pd.DataFrame) -> str:
    """Deterministic, snapshot-friendly rendering of a DataFrame."""
    return df.to_string(na_rep="<NA>")


def _instance(ic: IndicatorConstraint, x_lower: float = 0.0, x_upper: float = 5.0):
    """Instance with continuous x (id=0, bound=[x_lower, x_upper]), binary y (id=1),
    and one indicator constraint at id=7."""
    x = DecisionVariable.continuous(0, lower=x_lower, upper=x_upper)
    y = DecisionVariable.binary(1)
    return Instance.from_components(
        decision_variables=[x, y],
        objective=x,
        constraints={},
        indicator_constraints={7: ic},
        sense=Instance.MINIMIZE,
    )


def test_convert_inequality_emits_only_upper_big_m():
    """`y=1 → x - 2 <= 0` with x in [0, 5]: upper = 3 > 0, lower side irrelevant.
    Expect exactly one new constraint `x + 3 y - 5 <= 0` and the original indicator
    recorded on the removed side."""
    y = DecisionVariable.binary(1)
    ic = IndicatorConstraint(
        indicator_variable=y,
        function=DecisionVariable.continuous(0, lower=0, upper=5) - 2,
        equality=Equality.LessThanOrEqualToZero,
    )
    instance = _instance(ic)

    new_ids = instance.convert_indicator_to_constraint(7)
    assert len(new_ids) == 1

    assert instance.indicator_constraints == {}
    removed = instance.removed_indicator_constraints[7]
    assert removed.removed_reason == "ommx.Instance.convert_indicator_to_constraint"
    assert removed.removed_reason_parameters["constraint_ids"] == str(new_ids[0])


def test_convert_equality_emits_both_sides():
    """`y=1 → x - 2 = 0` with x in [0, 5]: upper = 3, lower = -2 → both sides emit."""
    y = DecisionVariable.binary(1)
    ic = IndicatorConstraint(
        indicator_variable=y,
        function=DecisionVariable.continuous(0, lower=0, upper=5) - 2,
        equality=Equality.EqualToZero,
    )
    instance = _instance(ic)

    new_ids = instance.convert_indicator_to_constraint(7)
    assert len(new_ids) == 2
    assert instance.indicator_constraints == {}
    removed = instance.removed_indicator_constraints[7]
    assert removed.removed_reason_parameters["constraint_ids"] == ",".join(
        str(i) for i in new_ids
    )


def test_redundant_indicator_emits_no_constraint():
    """`y=1 → x - 10 <= 0` with x in [0, 5]: upper = -5 <= 0, so the constraint is
    already implied by the variable bounds. Conversion emits nothing but still
    moves the indicator to the removed side."""
    y = DecisionVariable.binary(1)
    ic = IndicatorConstraint(
        indicator_variable=y,
        function=DecisionVariable.continuous(0, lower=0, upper=5) - 10,
        equality=Equality.LessThanOrEqualToZero,
    )
    instance = _instance(ic)

    new_ids = instance.convert_indicator_to_constraint(7)
    assert new_ids == []
    assert instance.constraints == {}
    removed = instance.removed_indicator_constraints[7]
    assert removed.removed_reason_parameters["constraint_ids"] == ""


def test_infinite_bound_is_rejected_without_mutation():
    """Continuous x with default (infinite) bound cannot be Big-M converted.
    The call must raise and the instance must be untouched."""
    y = DecisionVariable.binary(1)
    # DecisionVariable.continuous() with no bound arguments → unbounded.
    x = DecisionVariable.continuous(0)
    ic = IndicatorConstraint(
        indicator_variable=y,
        function=x,
        equality=Equality.LessThanOrEqualToZero,
    )
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x,
        constraints={},
        indicator_constraints={7: ic},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(RuntimeError, match="non-finite"):
        instance.convert_indicator_to_constraint(7)

    assert 7 in instance.indicator_constraints
    assert instance.constraints == {}
    assert instance.removed_indicator_constraints == {}


def test_convert_all_is_atomic_on_error():
    """Bulk convert fails before applying any conversions if one indicator is
    un-convertible."""
    y = DecisionVariable.binary(10)
    x_bounded = DecisionVariable.continuous(0, lower=0, upper=5)
    x_unbounded = DecisionVariable.continuous(1)  # infinite bound
    ic_ok = IndicatorConstraint(
        indicator_variable=y,
        function=x_bounded - 2,
        equality=Equality.LessThanOrEqualToZero,
    )
    ic_bad = IndicatorConstraint(
        indicator_variable=y,
        function=x_unbounded,
        equality=Equality.LessThanOrEqualToZero,
    )
    instance = Instance.from_components(
        decision_variables=[x_bounded, x_unbounded, y],
        objective=x_bounded,
        constraints={},
        indicator_constraints={1: ic_ok, 2: ic_bad},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(RuntimeError, match="non-finite"):
        instance.convert_all_indicators_to_constraints()

    # Nothing was applied.
    assert set(instance.indicator_constraints) == {1, 2}
    assert instance.constraints == {}
    assert instance.removed_indicator_constraints == {}


def test_removed_indicator_constraints_df_surfaces_reason_and_ids(snapshot):
    """`constraints_df(kind="indicator", removed=True)` surfaces the
    reason and comma-joined new-constraint IDs on removed rows."""
    y = DecisionVariable.binary(1)
    ic = IndicatorConstraint(
        indicator_variable=y,
        function=DecisionVariable.continuous(0, lower=0, upper=5) - 2,
        equality=Equality.EqualToZero,
    )
    instance = _instance(ic)
    instance.convert_indicator_to_constraint(7)
    assert _df_snap(instance.constraints_df(kind="indicator", removed=True)) == snapshot
