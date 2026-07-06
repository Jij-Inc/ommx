import pytest

from ommx import DecisionVariable, Instance, Linear, Parameter, ParametricInstance
from ommx.display import FunctionDisplay


def test_format_function_accepts_to_function_and_returns_full_text():
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

    assert instance.format_function(x[0] + 2 * x[1]) == "x[0] + 2*x[1]"


def test_display_function_returns_bounded_function_display_by_default():
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
    assert str(display).startswith("x[0] + x[1]")


def test_function_display_html_escapes_labels_and_reports_truncation():
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


def test_parametric_instance_format_function_uses_parameter_labels():
    x = DecisionVariable.binary(0, name="x")
    p = Parameter(100, name="p", parameters={"scenario": "base"})
    instance = ParametricInstance.from_components(
        sense=Instance.MINIMIZE,
        objective=x + p,
        decision_variables=[x],
        constraints={},
        parameters=[p],
    )

    assert instance.format_function(x + p) == "x + p[scenario=base]"


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
