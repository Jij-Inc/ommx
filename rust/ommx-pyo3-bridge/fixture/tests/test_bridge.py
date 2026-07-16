"""End-to-end tests for the bridge's independently built consumer fixture."""

import importlib
import importlib.machinery
from pathlib import Path
import subprocess
import sys
import textwrap

import ommx
import ommx._ommx_rust
import ommx_pyo3_bridge_fixture as fixture


def test_reconstruction_endpoints_stay_binding_private() -> None:
    endpoints = (
        "_pyo3_bridge_function_from_bytes",
        "_pyo3_bridge_constraint_from_bytes",
        "_pyo3_bridge_decision_variable_from_bytes",
    )

    for endpoint in endpoints:
        assert hasattr(ommx._ommx_rust, endpoint)
        assert not hasattr(ommx, endpoint)


def test_fixture_and_ommx_are_distinct_extension_modules() -> None:
    fixture_native = importlib.import_module(
        "ommx_pyo3_bridge_fixture.ommx_pyo3_bridge_fixture"
    )
    fixture_path = Path(fixture_native.__file__).resolve()
    ommx_path = Path(ommx._ommx_rust.__file__).resolve()

    assert fixture_path != ommx_path
    assert any(
        str(fixture_path).endswith(suffix)
        for suffix in importlib.machinery.EXTENSION_SUFFIXES
    )
    assert any(
        str(ommx_path).endswith(suffix)
        for suffix in importlib.machinery.EXTENSION_SUFFIXES
    )


def test_function_is_canonical_and_preserves_terms() -> None:
    value = fixture.function()

    assert type(value) is ommx.Function
    assert value.linear_terms == {7: 1.0}
    assert value.constant_term == -3.0


def test_constraint_is_canonical_and_preserves_context() -> None:
    value = fixture.constraint()

    assert type(value) is ommx.Constraint
    assert value.equality == ommx.Equality.LessThanOrEqualToZero
    assert value.function.linear_terms == {7: 1.0}
    assert value.function.constant_term == -3.0
    assert value.name == "capacity"
    assert value.subscripts == [2, 5]
    assert value.parameters == {"axis": "row"}
    assert value.description == "bridge fixture"
    assert len(value.provenance) == 1
    assert value.provenance[0].kind == ommx.ProvenanceKind.OneHotConstraint
    assert value.provenance[0].original_id == 23


def test_decision_variable_is_canonical_and_preserves_owner_side_data() -> None:
    value = fixture.decision_variable()

    assert type(value) is ommx.DecisionVariable
    assert value.id == 7
    assert value.kind == 2
    assert value.bound.lower == -2.0
    assert value.bound.upper == 8.0
    assert value.name == "x"
    assert value.subscripts == [2, 5]
    assert value.parameters == {"axis": "row"}
    assert value.description == "bridge fixture"


def test_instance_is_canonical_and_preserves_root_owned_data() -> None:
    value = fixture.instance()

    assert type(value) is ommx.Instance
    assert value.sense == ommx.Instance.MINIMIZE
    assert value.objective.linear_terms == {7: 1.0}
    assert value.objective.constant_term == -3.0
    assert len(value.decision_variables) == 1
    variable = value.decision_variables[0]
    assert variable.id == 7
    assert variable.kind == 2
    assert variable.bound.lower == -2.0
    assert variable.bound.upper == 8.0
    assert variable.name == "instance_x"
    assert variable.subscripts == [9]


def test_generated_stub_uses_canonical_ommx_types() -> None:
    stub = (
        Path(__file__).parents[1].joinpath("ommx_pyo3_bridge_fixture.pyi").read_text()
    )

    assert "import ommx" in stub
    assert "def function() -> ommx.Function:" in stub
    assert "def constraint() -> ommx.Constraint:" in stub
    assert "def decision_variable() -> ommx.DecisionVariable:" in stub
    assert "def instance() -> ommx.Instance:" in stub
    assert "typing.Any" not in stub
    assert "override_return_type" not in stub


def test_missing_python_bridge_endpoint_has_a_clear_error() -> None:
    program = textwrap.dedent(
        """
        import sys
        import types

        import ommx_pyo3_bridge_fixture as fixture

        fake_ommx = types.ModuleType("ommx")
        fake_ommx.__path__ = []
        fake_rust = types.ModuleType("ommx._ommx_rust")
        fake_ommx._ommx_rust = fake_rust
        fake_ommx.Instance = type("Instance", (), {})
        sys.modules["ommx"] = fake_ommx
        sys.modules["ommx._ommx_rust"] = fake_rust

        def assert_missing_capability(call, capability):
            try:
                call()
            except ImportError as error:
                message = str(error)
                assert "required bridge capability" in message
                assert capability in message
                assert "compatible" in message
            else:
                raise AssertionError("bridge conversion unexpectedly succeeded")

        assert_missing_capability(
            fixture.function,
            "_pyo3_bridge_function_from_bytes",
        )
        assert_missing_capability(
            fixture.instance,
            "ommx.Instance.from_v2_bytes",
        )
        """
    )
    result = subprocess.run(
        [sys.executable, "-c", program],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
