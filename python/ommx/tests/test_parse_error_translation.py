from collections.abc import Callable
from pathlib import Path

import pytest

import ommx
from ommx import _ommx_rust


@pytest.mark.parametrize(
    ("decoder", "root"),
    [
        pytest.param(ommx.Instance.from_v1_bytes, "ommx.v1.Instance", id="instance-v1"),
        pytest.param(ommx.Instance.from_v2_bytes, "ommx.v2.Instance", id="instance-v2"),
        pytest.param(
            ommx.ParametricInstance.from_v1_bytes,
            "ommx.v1.ParametricInstance",
            id="parametric-instance-v1",
        ),
        pytest.param(
            ommx.ParametricInstance.from_v2_bytes,
            "ommx.v2.ParametricInstance",
            id="parametric-instance-v2",
        ),
        pytest.param(ommx.Solution.from_v1_bytes, "ommx.v1.Solution", id="solution-v1"),
        pytest.param(ommx.Solution.from_v2_bytes, "ommx.v2.Solution", id="solution-v2"),
        pytest.param(
            ommx.SampleSet.from_v1_bytes, "ommx.v1.SampleSet", id="sample-set-v1"
        ),
        pytest.param(
            ommx.SampleSet.from_v2_bytes, "ommx.v2.SampleSet", id="sample-set-v2"
        ),
        pytest.param(ommx.State.from_v1_bytes, "ommx.v1.State", id="state-v1"),
        pytest.param(ommx.Samples.from_v1_bytes, "ommx.v1.Samples", id="samples-v1"),
        pytest.param(
            _ommx_rust.Parameters.from_v1_bytes,
            "ommx.v1.Parameters",
            id="parameters-v1",
        ),
    ],
)
def test_malformed_protobuf_bytes_raise_value_error(
    decoder: Callable[[bytes], object], root: str
) -> None:
    with pytest.raises(ValueError) as exc_info:
        decoder(b"\x80")

    message = str(exc_info.value)
    assert f"{root}[bytes]" in message
    assert message.count("Cannot decode as a Protobuf Message") == 1


def test_qplib_syntax_error_raises_value_error(tmp_path: Path) -> None:
    path = tmp_path / "invalid.qplib"
    path.write_text("MIPBAND\nNOT_A_VALID_TYPE\n", encoding="utf-8")

    with pytest.raises(ValueError, match="QPLIB parse error at line 2"):
        ommx.Instance.load_qplib(str(path))


def test_semantic_protobuf_parse_error_raises_value_error() -> None:
    with pytest.raises(ValueError, match="OMMX Message parse error"):
        ommx.Instance.from_v1_bytes(b"")


def test_python_argument_extraction_remains_type_error() -> None:
    with pytest.raises(TypeError):
        ommx.Instance.from_v1_bytes("not bytes")  # type: ignore[arg-type]


def test_mps_syntax_error_remains_runtime_error(tmp_path: Path) -> None:
    path = tmp_path / "invalid.mps"
    path.write_text("NOT_A_HEADER\n", encoding="utf-8")

    with pytest.raises(RuntimeError, match="invalid MPS header"):
        ommx.Instance.load_mps(str(path))


@pytest.mark.parametrize(
    "loader",
    [ommx.Instance.load_qplib, ommx.Instance.load_mps],
    ids=["qplib", "mps"],
)
def test_missing_input_file_remains_runtime_error(
    loader: Callable[[str], ommx.Instance], tmp_path: Path
) -> None:
    with pytest.raises(RuntimeError):
        loader(str(tmp_path / "missing"))
