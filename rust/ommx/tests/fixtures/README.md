# Test fixtures

## QPLIB import and solution regression tests

`QPLIB_0018.qplib`, `QPLIB_0018.sol`, `QPLIB_0681.qplib`, and
`QPLIB_0681.sol` were downloaded unchanged on 2026-09-11 from
`https://qplib.zib.de/qplib/` and `https://qplib.zib.de/sol/` respectively.

QPLIB is licensed under [CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/).
Attribution: Furini et al., *QPLIB: a library of quadratic programming
instances*, Mathematical Programming Computation,
[DOI: 10.1007/s12532-018-0147-4](https://doi.org/10.1007/s12532-018-0147-4).
Instance donors: Andrea Scozzari (0018), Maria Saumell (0681).

The published states check quadratic objectives (0018) and quadratic
constraints with mixed continuous/binary variables (0681), without solving or
network access. Expected objective values come from the published `.sol`
files and were independently checked against the corresponding official GAMS
models. `quadratic_scaling.qplib` is a synthetic two-variable fixture that
separates diagonal, cross, linear, and constant terms and both bound directions.

## Auth e2e

`htpasswd` is the bcrypt-hashed credential file consumed by the
`registry:2` test container in `tests/auth_e2e.rs`. It encodes a single
user `alice` with password `secret`, generated via:

```
htpasswd -nbB alice secret > htpasswd
```

The hash is committed (rather than regenerated at runtime) so the test
suite does not need a bcrypt dependency and so the file is byte-stable
across CI runs.

The credentials are intentionally weak — they only ever authenticate
against an ephemeral testcontainer that is torn down at the end of each
test, so there is no security relevance.
