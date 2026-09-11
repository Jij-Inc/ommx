# QPLIB distribution adopted by OMMX 2.8

Repository: `ghcr.io/jij-inc/ommx/v2.8/qplib:{numeric-tag}`.

The [inventory](qplib.csv) records all 453 source instances, with their source
file checksums, model counts, Instance layer digests, and remote manifest
digests. Every input was converted successfully. The instance tags match the
previous unversioned distribution, whose references remain unchanged.
MIPLIB continues to use the v2.7 distribution.

## Source and generator

- Source: the official [QPLIB archive](https://qplib.zib.de/qplib.zip), linked
  from the [QPLIB website](https://qplib.zib.de/).
- Archive size: 774,972,035 bytes; 453 `.qplib` members.
- Archive SHA-256: `b3596e1264ed57c5f6a44e822679f5c9138e1985fe74bc8341c3becbc666b9fd`.
- Generator: repository commit `ed55a6ad9d0244469a185860750d9695ebd3f730`, built
  with `cargo build --locked --release -p dataset` and OMMX 2.8.0.
- Parser: the v2 parser with the quadratic coefficient correction from
  [#1209](https://github.com/Jij-Inc/ommx/pull/1209), backported from
  [#1208](https://github.com/Jij-Inc/ommx/pull/1208).

Each Artifact retains the authors, CC-BY-4.0 license, and QPLIB annotations
from the repository's metadata snapshot. Model counts describe the generated
OMMX Instance; QPLIB counts a two-sided constraint as one source constraint.
The `org.ommx.qplib.parser_version` annotation records `2.8.0`.

## Model comparisons

The [comparison record](model-equivalence.json) binds the source files and
generated Instance digests to independent checks against the official GAMS
models. No solver or downloaded code is executed.

For 0018 and 0681, all 4,336 objective and constraint comparisons at eight
states per instance agree within tolerance. At the published solutions:

| Instance | Artifact objective | Feasible at `atol=1e-8` |
| --- | ---: | --- |
| 0018 | -6.386014981598351 | Yes |
| 0681 | 45.24444816648748 | Yes |

Variable and continuous-variable counts match the metadata for all 453
instances. In 3525, the source declares 1,662 integer variables; 831 have
bounds `[0, 1]` and are represented as binary variables in OMMX. All 1,749
variables have the same integrality and bounds as the GAMS source. The
comparison uses the documented [GAMS variable defaults](https://www.gams.com/latest/docs/UG_Variables.html#UG_Variables_VariableTypes)
and every explicit `.lo`, `.up`, and `.fx` assignment.

To repeat the comparison from the repository root with the v2 environment:

```sh
uv run --no-sync python rust/dataset/distributions/v2.8/check_models.py \
  --archive /path/to/qplib.zip \
  --report rust/dataset/distributions/v2.8/qplib.csv \
  --registry /path/to/qplib-v2.8-registry \
  --output /path/to/model-equivalence.json
```

## Publication identity

Uploads use `ommx push`. Every remote manifest is compared with the parsed
local manifest, and its Instance layer digest must match the generation
report. Existing remote references are accepted only when their content
matches; different content stops publication rather than being overwritten.

OMMX v2 can reserialize manifest JSON during transfer, changing key order and
the manifest digest. Compare cached Instance layer digests and remote manifest
digests separately against the inventory.
