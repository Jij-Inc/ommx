import pandas as pd
import pytest

from ommx.artifact import ArtifactDraft


@pytest.mark.parametrize(
    "frame",
    [
        pd.DataFrame(
            {"count": [1, 2], "value": [0.5, 1.5]},
            index=pd.Index(["left", "right"], name="row"),
        ),
        pd.DataFrame(
            {
                "count": pd.Series([1, None], dtype="Int64"),
                "label": pd.Series(["first", None], dtype="string"),
            }
        ),
    ],
    ids=["named-index", "nullable-columns"],
)
def test_dataframe_parquet_round_trip(frame: pd.DataFrame):
    draft = ArtifactDraft.new_anonymous()
    descriptor = draft.add_dataframe(frame)
    artifact = draft.commit()

    assert descriptor.media_type == "application/vnd.apache.parquet"
    pd.testing.assert_frame_equal(artifact.get_dataframe(descriptor), frame)
