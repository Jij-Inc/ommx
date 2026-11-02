# Legacy Artifact compatibility fixture

`pyarrow-22.parquet` is the single external fixture for the former OMMX
DataFrame backend. It was written with PyArrow 22.0.0 and pandas 2.3.3.
It contains the DataFrame returned by `legacy_dataframe()` in
`test_artifact_parquet.py`, including nullable integers, a full-range unsigned
integer, binary/text data, dates, timestamps, a timezone and a named index.

Its purpose is to check OMMX's interpretation of an existing Artifact's pandas
metadata and values. It is not a Parquet format or compression conformance
suite. File-format handling and compression are the parquet crate's
responsibility; those cases do not need fixed binary files in this repository.

Test ownership:

| Owner | Contract | Test |
| --- | --- | --- |
| pandas conversion | dtype, missing values, labels, indexes, categories and metadata interpretation | `test_dataframe_conversion.py`, no Parquet or registry |
| PyO3 boundary | typed Python/Rust extraction and reconstruction, including `u64::MAX` | `python/ommx/src/artifact/dataframe.rs`, no Parquet or registry |
| Rust table adapter | table invariants, nullable column/type mapping and supported-schema policy | `artifact/parquet/table.rs`, generated tables/schemas |
| Artifact | layer storage, annotations, descriptor ownership, mutation safety and error translation | `artifact/parquet.rs`, `test_artifact_parquet.py` |
| Old OMMX backend compatibility | existing DataFrame layer values and pandas metadata | this fixture, read through `Artifact.get_dataframe()` |
| Dependency boundary | operation without PyArrow or fastparquet | isolated-process test in `test_artifact_parquet.py` |

Regeneration uses a temporary development dependency, not a runtime or test
dependency:

```sh
uv run --no-sync --with pyarrow==22.0.0 python - <<'PY'
import importlib.util
from pathlib import Path
import pyarrow as pa
import pyarrow.parquet as pq

source = Path("python/ommx-tests/tests/test_artifact_parquet.py")
spec = importlib.util.spec_from_file_location("artifact_parquet_tests", source)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
pq.write_table(
    pa.Table.from_pandas(module.legacy_dataframe()),
    source.parent / "fixtures" / "parquet" / "pyarrow-22.parquet",
    compression="snappy",
    row_group_size=2,
)
PY
```
