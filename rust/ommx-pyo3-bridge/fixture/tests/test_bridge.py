"""End-to-end tests for the bridge's independently built consumer fixture."""

import importlib
import importlib.machinery
import json
from pathlib import Path
import subprocess
import sys
import textwrap

import ommx
import ommx._ommx_rust
import ommx_pyo3_bridge_fixture as fixture


V0_CONTRACT_PATH = Path(__file__).parents[2] / "tests/data/protocol_v0.json"
V0_CONTRACT = json.loads(V0_CONTRACT_PATH.read_text())


def assert_component_function(value: ommx.Function) -> None:
    assert type(value) is ommx.Function
    assert value.linear_terms == {7: 1.0}
    assert value.constant_term == -3.0


def assert_component_constraint(value: ommx.Constraint) -> None:
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


def assert_component_decision_variable(value: ommx.DecisionVariable) -> None:
    assert type(value) is ommx.DecisionVariable
    assert value.id == 7
    assert value.kind == 2
    assert value.bound.lower == -2.0
    assert value.bound.upper == 8.0
    assert value.name == "x"
    assert value.subscripts == [2, 5]
    assert value.parameters == {"axis": "row"}
    assert value.description == "bridge fixture"


def assert_component_instance(value: ommx.Instance) -> None:
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


def test_reconstruction_endpoints_stay_binding_private() -> None:
    endpoints = (
        "_pyo3_bridge_v0_function_from_bytes",
        "_pyo3_bridge_v0_constraint_from_bytes",
        "_pyo3_bridge_v0_decision_variable_from_bytes",
    )

    for endpoint in endpoints:
        assert hasattr(ommx._ommx_rust, endpoint)
        assert not hasattr(ommx, endpoint)

    unversioned_endpoints = (
        "_pyo3_bridge_function_from_bytes",
        "_pyo3_bridge_constraint_from_bytes",
        "_pyo3_bridge_decision_variable_from_bytes",
    )
    for endpoint in unversioned_endpoints:
        assert not hasattr(ommx._ommx_rust, endpoint)


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
    assert_component_function(fixture.function())


def test_constraint_is_canonical_and_preserves_context() -> None:
    assert_component_constraint(fixture.constraint())


def test_decision_variable_is_canonical_and_preserves_owner_side_data() -> None:
    assert_component_decision_variable(fixture.decision_variable())


def test_instance_is_canonical_and_preserves_root_owned_data() -> None:
    assert_component_instance(fixture.instance())


def test_frozen_v0_payloads_reconstruct_canonical_values() -> None:
    function = V0_CONTRACT["function"]
    assert_component_function(
        getattr(ommx._ommx_rust, function["endpoint"])(
            bytes.fromhex(function["payload"])
        )
    )

    constraint = V0_CONTRACT["constraint"]
    assert_component_constraint(
        getattr(ommx._ommx_rust, constraint["endpoint"])(
            bytes.fromhex(constraint["constraint"]),
            bytes.fromhex(constraint["context"]),
        )
    )

    decision_variable = V0_CONTRACT["decision_variable"]
    assert_component_decision_variable(
        getattr(ommx._ommx_rust, decision_variable["endpoint"])(
            decision_variable["id"],
            bytes.fromhex(decision_variable["decision_variable"]),
            bytes.fromhex(decision_variable["label"]),
        )
    )

    instance = V0_CONTRACT["instance"]
    assert instance["capability"] == "ommx.Instance.from_v2_bytes"
    assert_component_instance(
        ommx.Instance.from_v2_bytes(bytes.fromhex(instance["payload"]))
    )


def test_sender_matches_frozen_v0_endpoint_signatures_and_payloads() -> None:
    program = textwrap.dedent(
        """
        import json
        from pathlib import Path
        import sys
        import types

        contract = json.loads(Path(sys.argv[1]).read_text())
        fake_ommx = types.ModuleType("ommx")
        fake_ommx.__path__ = []
        fake_rust = types.ModuleType("ommx._ommx_rust")
        fake_ommx._ommx_rust = fake_rust

        function = contract["function"]
        def receive_function(payload):
            assert payload.hex() == function["payload"]
            return "function"
        setattr(fake_rust, function["endpoint"], receive_function)

        constraint = contract["constraint"]
        def receive_constraint(payload, context):
            assert payload.hex() == constraint["constraint"]
            assert context.hex() == constraint["context"]
            return "constraint"
        setattr(fake_rust, constraint["endpoint"], receive_constraint)

        decision_variable = contract["decision_variable"]
        def receive_decision_variable(id, payload, label):
            assert id == decision_variable["id"]
            assert payload.hex() == decision_variable["decision_variable"]
            assert label.hex() == decision_variable["label"]
            return "decision_variable"
        setattr(
            fake_rust,
            decision_variable["endpoint"],
            receive_decision_variable,
        )

        instance = contract["instance"]
        assert instance["capability"] == "ommx.Instance.from_v2_bytes"
        class Instance:
            @staticmethod
            def from_v2_bytes(payload):
                assert payload.hex() == instance["payload"]
                return "instance"
        fake_ommx.Instance = Instance

        sys.modules["ommx"] = fake_ommx
        sys.modules["ommx._ommx_rust"] = fake_rust

        import ommx_pyo3_bridge_fixture as fixture

        assert fixture.function() == "function"
        assert fixture.constraint() == "constraint"
        assert fixture.decision_variable() == "decision_variable"
        assert fixture.instance() == "instance"
        """
    )
    result = subprocess.run(
        [sys.executable, "-c", program, str(V0_CONTRACT_PATH)],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr


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
            "_pyo3_bridge_v0_function_from_bytes",
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
