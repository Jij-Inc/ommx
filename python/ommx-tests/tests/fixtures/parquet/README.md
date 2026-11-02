# Parquet interoperability fixtures

`pyarrow-22.parquet` was written with PyArrow 22.0.0 and pandas 2.3.3,
using Snappy compression and two-row row groups. It contains the DataFrame
returned by `legacy_dataframe()` in `test_artifact_parquet.py`:
nullable signed/unsigned integers (including `u64::MAX`), booleans, floats,
UTF-8, binary values, dates, nanosecond timestamps, a time zone, and a named
string index. It exercises reading artifacts produced by the former backend.

`nested.parquet` was written using
`pyarrow.parquet.write_table(pyarrow.table({"nested": [[1, 2], None]}), path)`.
It verifies that an unsupported nested schema is rejected explicitly.

The codec-named files contain an unannotated required `int64` column `i`
(`[1, 2, 3]`) and an optional UTF-8 column `s` (`["日本語", None, ""]`),
written in two-row groups with the named compression codec. `empty.parquet`
contains `pa.Table.from_pandas(pd.DataFrame())`. `categorical.parquet` contains
an ordered categorical column with values `["b", "a"]` and categories
`["b", "a", "unused"]`; reading it must reject loss of category information.

The tests consume these fixed files without installing PyArrow. To regenerate
them, use the project environment with a temporary PyArrow dependency:

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
root = source.parent / "fixtures" / "parquet"
pq.write_table(pa.Table.from_pandas(module.legacy_dataframe()),
               root / "pyarrow-22.parquet", compression="snappy", row_group_size=2)
pq.write_table(pa.table({"nested": [[1, 2], None]}), root / "nested.parquet")
table = pa.Table.from_arrays(
    [pa.array([1, 2, 3]), pa.array(["日本語", None, ""])],
    schema=pa.schema([pa.field("i", pa.int64(), nullable=False), pa.field("s", pa.string())]),
)
for codec in ["snappy", "gzip", "brotli", "lz4", "zstd", "none"]:
    pq.write_table(table, root / f"{codec}.parquet", compression=codec, row_group_size=2)
import pandas as pd
pq.write_table(pa.Table.from_pandas(pd.DataFrame()), root / "empty.parquet")
df = pd.DataFrame({"category": pd.Categorical(["b", "a"], categories=["b", "a", "unused"], ordered=True)})
pq.write_table(pa.Table.from_pandas(df), root / "categorical.parquet")
PY
```
