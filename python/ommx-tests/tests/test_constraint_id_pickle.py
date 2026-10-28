"""Serialization preserves constraint families without integer coercion."""

import pickle
import subprocess
import sys
from io import BytesIO

import pandas as pd
import pytest

from ommx import Instance, ParametricInstance
from test_live_constraint_collections import FAMILIES, make_model


@pytest.mark.parametrize("id_type", [family[1] for family in FAMILIES])
@pytest.mark.parametrize("value", [0, 5, 2**64 - 1])
@pytest.mark.parametrize("protocol", range(pickle.HIGHEST_PROTOCOL + 1))
def test_id_pickle_preserves_family_and_value(id_type, value, protocol):
    key = id_type(value)
    restored = pickle.loads(pickle.dumps(key, protocol=protocol))
    assert type(restored) is id_type
    assert restored == key
    assert hash(restored) == hash(key)
    assert restored != value
    for _, other_type, _ in FAMILIES:
        if other_type is not id_type:
            assert restored != other_type(value)
    with pytest.raises(TypeError):
        int(restored)


def test_mixed_id_mapping_pickle_in_fresh_process():
    ids = [id_type(5) for _, id_type, _ in FAMILIES]
    payload = pickle.dumps({key: [key] for key in ids})
    # Loading must resolve the public class without pre-importing the extension.
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import pickle, sys; rows = pickle.loads(sys.stdin.buffer.read()); "
            "assert len(rows) == 4; "
            "assert all(type(k) is type(v[0]) and k == v[0] for k, v in rows.items()); "
            "assert all(type(k).__module__ == 'ommx' for k in rows)",
        ],
        input=payload,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr.decode()
    assert pickle.loads(payload) == {key: [key] for key in ids}


@pytest.mark.parametrize("kind,id_type,_row_type", FAMILIES)
def test_constraint_dataframe_pickle(kind, id_type, _row_type):
    model = make_model(Instance)
    state = {0: 1, 1: 0}
    roots = [
        model,
        make_model(ParametricInstance),
        model.evaluate(state),
        model.evaluate_samples({9: state}),
    ]
    for root in roots:
        frame = root.constraints_df(kind=kind)
        # Also cover a typed ID column, as used by long-format sidecar tables.
        frame["constraint_id"] = frame.index
        buffer = BytesIO()
        frame.to_pickle(buffer)
        buffer.seek(0)
        restored = pd.read_pickle(buffer)
        pd.testing.assert_frame_equal(restored, frame)
        assert type(restored.index[0]) is id_type
        assert type(restored.at[id_type(5), "constraint_id"]) is id_type
