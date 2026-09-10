# MIPLIB 2017 distribution adopted by OMMX 2.7

Repository: `ghcr.io/jij-inc/ommx/v2.7/miplib2017:{instance-name}`.

The [inventory](miplib2017.csv) records all 1,065 source instances: 1,012
Artifacts uploaded to this repository and 53 parser failures. The successful
instance names are exactly the same as in the previous unversioned repository.
The previous repository was not modified.

## Source and generator

- Source: the official [MIPLIB collection archive](https://miplib.zib.de/downloads/collection.zip),
  linked from the [MIPLIB downloads page](https://miplib.zib.de/download.html).
- Archive size: 3,776,190,540 bytes; 1,065 `.mps.gz` members.
- Archive SHA-256: `02b3933f7c6342f5be371070c6d79b9ffe580cd84275ec86e63eaa0f2d406700`.
- Generator: repository commit `297a903b2913d2cb254d2cf30c229f48209e2b95`,
  built with `cargo build --release --locked -p dataset` and OMMX 2.7.0.
- Parser: the existing v2 parser including the integer-default-bound fix from
  [#1203](https://github.com/Jij-Inc/ommx/pull/1203); no additional syntax support.

The generator retains source authors, licenses, and MIPLIB annotations on each
Instance layer. Inventory counts describe the generated OMMX model. Empty
counts or digests on failed rows mean no Artifact was published for that input.

Each successful row records the Instance layer digest and the remote manifest
digest obtained after uploading the original OCI layout. The Instance digest
identifies model bytes; the manifest also covers packaging metadata. SDK 2.x
can reserialize manifest JSON when caching an Artifact, so compare cached
Instance layer digests and remote manifest digests separately.

## Inputs rejected by the current parser

| Rejected construct | Instances |
| --- | ---: |
| `INDICATORS` section | 40 |
| Multiple `N` rows | 10 |
| `LAZYCONS` section | 2 |
| Six-field `COLUMNS` record (`neos-5044663-wairoa`) | 1 |

The inventory preserves each parser error verbatim. In particular, the parser
reports multiple `N` rows as multiple objectives; this is the parser's
diagnostic, not an independent classification of those source problems.

## Interpreting counts

Total variable and continuous-variable counts match the MIPLIB metadata for
all published instances. Two kinds of representation differences remain.

### Integers with binary domains

| Instance | MIPLIB general integers | OMMX binary variables |
| --- | ---: | ---: |
| `ex9` | 10,404 | 10,404 |
| `ex10` | 17,680 | 17,680 |

The original MPS files explicitly bound every integer variable in these two
instances to `[0, 1]`. The v2 parser represents them as binary variables.
[Domain comparison records](domain-equivalence.json) bind the original
compressed MPS checksum and generated Instance digest to a comparison with
HiGHS 1.13.1: every named variable has the same integrality and lower/upper
bounds. HiGHS only reads the source model; no optimization or presolve is used.

To repeat this comparison, retain the generated local OCI layouts and run from
the repository root with the v2 Python environment:

```sh
uv run --no-sync --with highspy==1.13.1 python \
  rust/dataset/distributions/v2.7/check_domains.py \
  --archive /path/to/collection.zip \
  --report rust/dataset/distributions/v2.7/miplib2017.csv \
  --registry /path/to/miplib-v2.7-registry \
  --output /path/to/domain-equivalence.json
```

### Ranged rows

OMMX represents a ranged MPS row using two inequalities. For these nine
instances, the increase in constraint count equals the number of distinct
source rows in `RANGES`:

| Instance | MIPLIB constraints | OMMX constraints | Ranged rows |
| --- | ---: | ---: | ---: |
| `gus-sch` | 5,984 | 5,994 | 10 |
| `mod011` | 4,480 | 4,497 | 17 |
| `neos-1140050` | 3,795 | 4,230 | 435 |
| `neos-1223462` | 5,890 | 5,925 | 35 |
| `neos-3045796-mogo` | 2,226 | 2,245 | 19 |
| `ns1644855` | 40,698 | 50,698 | 10,000 |
| `ns930473` | 23,240 | 45,660 | 22,420 |
| `p2m2p1m1p0n100` | 1 | 2 | 1 |
| `rentacar` | 6,803 | 6,805 | 2 |

## Original regression instance

The published `neos-2626858-aoos` contains 524 variables: 209 binary and 315
general integer variables, with 342 constraints. The 17 variables
`C0493` through `C0508` and `C0524` are binary with bounds `[0, 1]`.
These domains are also present when loading the published Artifact through
the SDK's default `ommx.dataset.miplib2017` path.

The archive's `neos-2626858-aoos.mps.gz` is byte-for-byte identical to the
regression fixture in `rust/ommx/tests/data/mps/`, with SHA-256
`7bea170d9d1a006bae4abad8483ba86e9202eb358701edf7f9012feb0c6cd101`.
