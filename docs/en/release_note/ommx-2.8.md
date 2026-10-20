# OMMX Python SDK 2.8.x

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_2.8.0-blue?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-2.8.0)

See the GitHub Releases above for full details. The following summarizes the main changes.

## Bug Fixes

### Correct QPLIB quadratic coefficients and dataset Artifacts (2.8.0, [#1209](https://github.com/Jij-Inc/ommx/pull/1209))

`Instance.load_qplib()` and `ommx.qplib.load_file()` now apply the format's
factor of `1/2` to every quadratic coefficient in objectives and constraints,
including diagonal and cross terms. Previously, quadratic contributions were
doubled. Linear terms, constants, and bounds retain their source values.

`ommx.dataset.qplib()` now selects
`ghcr.io/jij-inc/ommx/v2.8/qplib:{tag}`, regenerated from the official source
files with the corrected parser. The Rust loader uses the same distribution.
Cached Artifacts from the old, unversioned repository do not override the new
distribution. Manually saved instances must be imported again from their
original `.qplib` files to receive the correction.

Published references are immutable, and patch releases retain their adopted
distribution. MIPLIB continues to use the v2.7 distribution. See the
[QPLIB tutorial](../tutorial/download_qplib_instance.ipynb) for the download path.

This correction changes the mathematical model, including objective values and
feasibility. Recompute results obtained from affected QPLIB instances after
upgrading. The 2.8.0 solver adapters require OMMX 2.8.0 or later in the v2 line.
