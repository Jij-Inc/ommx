# Extending variable domains and constraint families

V3 supports additive SDK extensions for numeric scalar variable domains and
constraint families. Generic identity, collection, and metadata APIs must keep
their types when the SDK adds a kind or family. Family-specific constructors
and accessors retain their precise types. Internal dispatch can remain exhaustive.

`Kind` and `ConstraintKind` are non-exhaustive. Consumers that interpret
semantics must handle supported values and explicitly reject the remainder:

```rust
use ommx::Kind;
fn supports(kind: Kind) -> bool {
    match kind {
        Kind::Binary | Kind::Integer | Kind::Continuous => true,
        _ => false,
    }
}
assert!(!supports(Kind::FiniteDomain));
```

Even listing every current kind must not establish exhaustiveness:

```compile_fail
use ommx::Kind;
fn closed(kind: Kind) {
    match kind {
        Kind::Binary | Kind::Integer | Kind::Continuous |
        Kind::SemiInteger | Kind::SemiContinuous | Kind::FiniteDomain => (),
    }
}
```

```compile_fail
use ommx::ConstraintKind;
fn closed(kind: ConstraintKind) {
    match kind {
        ConstraintKind::Regular | ConstraintKind::Indicator | ConstraintKind::OneHot |
        ConstraintKind::Sos1 => (),
    }
}
```

Python exposes `Kind` and `ConstraintKind` as nominal value classes
with known constants. Their type hints do not define a closed Enum or Literal
union. Existing comparisons, hashing and `Kind` protobuf conversion remain
available. Consumers must retain a rejecting fallback even after checking all
known constants. Mixed constraint collections expose the open
`ConstraintIDBase` and `ConstraintView` interfaces; concrete ID lookups keep
their family-specific row types.

`ConstraintKind` includes `Regular`, `Indicator`, `OneHot`, and `Sos1`. The same
selector is used for Python `constraints.of_kind(ConstraintKind.Sos1)` and
`constraints_df(kind=ConstraintKind.Sos1)`. Known selector constants carry the
family's ID type so filtered views retain precise row types, while a generic
selector retains the common ID and row interfaces. Strings are display labels,
not family-selector inputs. `active_constraint_kinds` includes regular rows;
special-constraint lowering rejects `Regular` before applying any conversion.

Transformation provenance follows the same rule: Rust `Provenance` is
non-exhaustive, with common `kind()` and `original_id()` metadata queries;
Python `ProvenanceKind` is an open value type.

```compile_fail
use ommx::Provenance;
fn closed(source: Provenance) {
    match source {
        Provenance::IndicatorConstraint(_) | Provenance::OneHotConstraint(_) |
        Provenance::Sos1Constraint(_) => (),
    }
}
```

## Ownership and unsupported semantics

A variable domain owns valid values, membership, normalization and applicable
value operations. Model roots own cross-object invariants: references exist,
constraint members have admissible domains, and coupled changes are validated
before mutation. Parsers, constructors, transformations and result restoration
must preserve the same invariants. Adding a family requires updating all root
validators and lifecycle paths, with round-trip and atomic rejection tests.

Adapters and formats declare their supported semantics and reject unsupported
ones. Never omit an unfamiliar constraint or reinterpret an unfamiliar variable
domain: either action can change the feasible region. SDK API compatibility,
wire reconstruction requirements and `InstanceClass` membership are independent
contracts. Old readers must reject semantic requirements they cannot reconstruct.

The current value contract remains a numeric scalar (`f64`, with sampled `f64`
values). Structured, interval-valued or set-valued variables require a separate
design for common identity/metadata versus scalar-specific values and operations.
This contract does not introduce arbitrary third-party domain registration.

Kind-specific statistics are also non-exhaustive: consumers may read known
fields or construct `VariableStatsByKind::default()` and set known counts.
Missing counts in serialized statistics default to zero.

## V3 migration

Rust consumers must add fallback arms to matches on the two public kind types.
Python consumers must use the known constants and a rejecting fallback, rather
than Enum exhaustiveness or `assert_never` to establish complete support.
Do not infer semantic support from integer comparisons or protobuf numbers.
Subsequent additions preserve these consumer types; they do not automatically
add new solver capabilities.

See [#1327](https://github.com/Jij-Inc/ommx/issues/1327) and the Spin use case in
[#1082](https://github.com/Jij-Inc/ommx/issues/1082).
