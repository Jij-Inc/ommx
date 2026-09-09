# OMMX Python SDK 2.7.x

Version 2.7.0 includes an MPS parsing correction that can change the
mathematical model loaded from the same file, including its feasible set.
Review affected models and previous solver results when upgrading.

The 2.7.0 solver adapter packages require OMMX 2.7.0 or later in the v2 line.

## Bug Fixes

### Preserve implicit binary bounds when loading MPS ([#1203](https://github.com/Jij-Inc/ommx/pull/1203))

`Instance.load_mps()` and `ommx.mps.load_file()` now give columns inside
`INTORG`/`INTEND` with no `BOUNDS` entry the implicit binary domain `[0, 1]`,
following the Gurobi/HiGHS MPS convention. Previously, these columns became
general integers with an infinite upper bound. This affected 17 variables in
`neos-2626858-aoos`, changing its binary-variable count from 209 to 192.

Explicit bound records remain effective, including `LO`/`LI 0` for nonnegative
integers with no upper limit. Previously generated Artifacts, including those
loaded through `ommx.dataset`, require separate regeneration from the source MPS;
updating the SDK does not repair stored instances.
