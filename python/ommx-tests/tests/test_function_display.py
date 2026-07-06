import pytest

from ommx import DecisionVariable, Instance, Linear, Parameter, ParametricInstance
from ommx.display import FunctionDisplay


def _display_snapshot(display: FunctionDisplay) -> str:
    return "\n".join(
        [
            str(display),
            "",
            f"repr={display!r}",
            f"total_terms={display.total_terms}",
            f"written_terms={display.written_terms}",
            f"omitted_terms={display.omitted_terms}",
            f"truncated_by_chars={display.truncated_by_chars}",
            f"truncated={display.truncated}",
        ]
    )


def test_format_function_accepts_to_function_and_returns_full_text(snapshot):
    x = [
        DecisionVariable.binary(0, name="x", subscripts=[0]),
        DecisionVariable.binary(1, name="x", subscripts=[1]),
    ]
    instance = Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=0,
        decision_variables=x,
        constraints={},
    )

    assert instance.format_function(x[0] + 2 * x[1]) == snapshot


def test_display_function_returns_bounded_function_display_by_default(snapshot):
    x = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(101)]
    instance = Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=0,
        decision_variables=x,
        constraints={},
    )

    display = instance.display_function(sum(x))

    assert isinstance(display, FunctionDisplay)
    assert display.total_terms == 101
    assert display.written_terms == 100
    assert display.omitted_terms == 1
    assert display.truncated
    assert _display_snapshot(display) == snapshot


def test_function_display_html_escapes_labels_and_reports_truncation(snapshot):
    x = [
        DecisionVariable.binary(0, name="<x&>"),
        DecisionVariable.binary(1, name="y"),
    ]
    instance = Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=0,
        decision_variables=x,
        constraints={},
    )

    display = instance.display_function(x[0] + x[1], max_terms=1)
    html = display._repr_html_()

    assert "<pre><code>&lt;x&amp;&gt;</code></pre>" in html
    assert "<x&>" not in html
    assert "showing 1 of 2 terms; 1 omitted" in html
    assert html == snapshot


def test_display_function_boundary_budgets(snapshot):
    x = [
        DecisionVariable.binary(0, name="alpha"),
        DecisionVariable.binary(1, name="beta"),
    ]
    instance = Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=0,
        decision_variables=x,
        constraints={},
    )

    zero_terms = instance.display_function(x[0] + x[1], max_terms=0)
    zero_chars = instance.display_function(x[0] + x[1], max_chars=0)
    partial_first_term = instance.display_function(x[0] + x[1], max_chars=3)

    assert (
        "\n\n".join(
            [
                "zero_terms\n" + _display_snapshot(zero_terms),
                "zero_chars\n" + _display_snapshot(zero_chars),
                "partial_first_term\n" + _display_snapshot(partial_first_term),
            ]
        )
        == snapshot
    )


def test_parametric_instance_format_function_uses_parameter_labels(snapshot):
    x = DecisionVariable.binary(0, name="x")
    p = Parameter(100, name="p", parameters={"scenario": "base"})
    instance = ParametricInstance.from_components(
        sense=Instance.MINIMIZE,
        objective=x + p,
        decision_variables=[x],
        constraints={},
        parameters=[p],
    )

    assert instance.format_function(x + p) == snapshot


def test_unknown_ids_error_even_beyond_preview_budget():
    x = DecisionVariable.binary(0, name="x")
    instance = Instance.from_components(
        sense=Instance.MINIMIZE,
        objective=0,
        decision_variables=[x],
        constraints={},
    )

    with pytest.raises(
        Exception, match="unknown decision variable ID VariableID\\(999\\)"
    ):
        instance.display_function(Linear(terms={0: 1.0, 999: 1.0}), max_terms=1)
