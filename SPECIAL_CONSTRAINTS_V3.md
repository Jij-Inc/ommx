# Special Constraints in OMMX v3 - Proto Schema Proposal

Status: **Proposal / WIP**

This document is a temporary design proposal. The current PR should merge this
proposal only, without changing protobuf files or generated code. Follow-up PRs
should first tighten the Rust SDK collection model, then introduce the new
protobuf schema. Once those PRs land, move the durable parts of this document
into API Reference text and protobuf comments, then delete this proposal file.

## Goal

Bring the OMMX v1 protobuf wire format in line with the current Rust/Python v3
runtime model for special constraints.

Terminology note: this document uses "v3" for the current Rust/Python SDK and
runtime model. It does not rename the protobuf package: the wire messages still
live under `ommx.v1`. Wire-format compatibility is governed by each top-level
message's `format_version`; this proposal discusses the semantic break from
`format_version == 0` to `format_version == 1`.

The current runtime already treats non-standard constraints as first-class
domain objects:

- `Instance` owns one `ConstraintCollection<T>` per constraint type:
  regular `Constraint`, `IndicatorConstraint`, `OneHotConstraint`, and
  `Sos1Constraint`.
- `Solution` and `SampleSet` likewise carry evaluated/sampled collections for
  all four constraint families.
- Python exposes these collections through `Instance.from_components(...)`,
  `indicator_constraints`, `one_hot_constraints`, `sos1_constraints`, and their
  evaluated/sampled counterparts.

The current protobuf schema is still v2-shaped:

- `Instance` serializes only `repeated Constraint constraints`, optional
  `ConstraintHints`, and `repeated RemovedConstraint removed_constraints`.
- `Solution` and `SampleSet` serialize only regular evaluated/sampled
  constraints.
- Serializing an `Instance` with special constraints is currently unsupported;
  serializing a `Solution` or `SampleSet` with special-constraint results is
  lossy.

This proposal replaces the hint-based and lossy wire representation with
first-class protobuf collections for regular, indicator, one-hot, and SOS1
constraints.

## Serialization Boundary

Starting with v3, distinguish protobuf schema components from SDK
serialization roots.

A serialization root is a message whose Rust/Python owner carries enough
context to validate OMMX domain invariants at the protobuf boundary. Public SDK
protobuf serialization is supported only for these roots:

- `Instance`
- `ParametricInstance`
- `Solution`
- `SampleSet`

These objects own the surrounding context needed to validate references and
sidecars: decision-variable definitions, variable kinds, constraint collection
IDs, removed-state ownership, modeling-label stores, constraint-context stores,
evaluation/sample ID universes, and top-level materialized summaries.

Leaf messages such as `Function`, polynomial pieces, individual constraints,
decision variables, named functions, modeling labels, constraint contexts, and
provenance records
are protobuf schema components, not SDK serialization roots. For example, a
standalone `Function` cannot validate whether referenced decision variables
exist or whether their kinds are compatible with the enclosing constraint.
Likewise, an individual constraint value does not own its collection ID,
removed-state placement, or sidecars.

The Rust and Python SDKs should therefore avoid exposing standalone public
protobuf round-trip APIs for leaf values. Parser/writer helpers for leaf
messages may exist internally, but the supported SDK validation contract remains
the top-level object that owns the surrounding domain context and invariants.
Other lightweight protobuf readers may still inspect sub-messages directly, but
they should treat such inspection as schema-level access rather than an
SDK-validated OMMX round-trip.

Top-level summaries such as `SampleSet.feasible` and
`SampleSet.feasible_relaxed` are part of this root-level contract. They are
materialized summaries for lightweight consumers that should not need a full
constraint evaluator just to preview sample feasibility. OMMX SDK readers should
still validate those fields against the sampled constraint data, and SDK writers
should emit the values computed from the root object.

## Domain Model

The runtime source of truth remains the collection owner, not the individual
constraint value.

- Constraint IDs are owned by the enclosing collection.
- ID spaces are independent per constraint family in the new
  `format_version == 1` collection fields. For example, regular constraint ID
  `1` and one-hot constraint ID `1` are distinct. This deliberately replaces the
  legacy global-uniqueness rule documented on the deprecated v2 `Constraint`
  message, which applies only to legacy fields read from `format_version == 0`
  payloads.
- Active and removed constraints are owned by the same
  `ConstraintCollection<T>`.
- `RemovedReason` is removal state in Rust, not part of the constraint's
  modeling context.
- `ModelingLabel` (`name`, `subscripts`, `parameters`, `description`) records
  the original mathematical-model notation and indexing context, such as
  `x[i, j]` or `flow limit` parameterized by `place`.
- Constraint-side labels and transformation lineage are grouped as
  `ConstraintContext` in Rust. Its `label` field is a `ModelingLabel`;
  `provenance` is separate transformation lineage and is not part of the
  modeling label itself.

The protobuf shape should therefore preserve collection ownership:

- No `id` field inside the new v3 constraint messages.
- One first-class collection per constraint family.
- No second source of truth between active entries, removed entries, removed
  reasons, and constraint contexts.

## Broader Normalization Tracking

Broader root-owned entity normalization is tracked separately in
[#958](https://github.com/Jij-Inc/ommx/issues/958). Resolve that issue before
the forward-incompatible protobuf format is finalized, so this proposal does
not become the only durable record for decisions about decision-variable,
named-function, parameter, and sample-ID ownership.

The exact wire shape is intentionally left as a follow-up decision. Two viable
shapes remain:

- A direct `map<uint64, T>` per constraint family where each value carries
  optional `context` and `removed_reason` fields. This
  keeps each map entry self-contained at the protobuf boundary.
- A collection wrapper message per constraint family that mirrors the Rust
  owner more closely, for example by separating active entries, removed entries,
  removed reasons, and a constraint-context store. This may
  better match the current Rust SoA representation after the SDK normalization
  pass.

On parse, the protobuf representation is split into:

- the ID from the map key,
- the intrinsic constraint data from the message,
- constraint contexts inserted into the collection context store,
- active vs. removed placement determined by either `removed_reason` presence
  in the direct-map shape or by the removed-entry set in the wrapper shape.

If the wrapper shape is chosen and constraint contexts are stored
as independent ID-keyed fields, unknown IDs must be rejected at parse/build
boundaries. Dropping orphan context entries would silently lose malformed input
and would make the wrapper's sidecars a second, weaker source of truth.

On write, the collection owner is the source of truth. If the direct-map shape
is chosen, active and removed maps are joined into one protobuf map, with
`removed_reason` set only for removed entries. If the wrapper shape is chosen,
the writer should emit the wrapper fields from the same collection owner without
duplicating state elsewhere.

## Compatibility Policy

- `format_version` is bumped from `0` to `1` in the same change.
- Forward compatibility is deliberately broken: an SDK whose
  `CURRENT_FORMAT_VERSION == 0` rejects v3 data through the existing
  `check_format_version` path.
- Backward compatibility is preserved on the v3 read side:
  - `format_version == 0` reads the legacy fields
    (`constraints`, `removed_constraints`, `constraint_hints`,
    `evaluated_constraints`, `SampleSet.constraints`, ...).
  - `ConstraintHints` are translated into first-class one-hot/SOS1 collections.
  - Indicator constraints cannot be recovered from v2 data because v2 had no
    indicator hint representation.
- `format_version == 1` reads the new collection fields as the source of truth.
  Legacy constraint fields should be empty in v3 data; non-empty legacy fields
  in a v3 payload should be rejected rather than silently merged, because that
  would create two competing sources of truth.
- v3 writers always emit the new fields and leave legacy fields empty.

## Naming

Use `RegularConstraint` as the wire name for the v3 form of the existing Rust
`ommx::Constraint`.

Rationale:

- The existing protobuf message name `Constraint` is the v2 message with an
  inline `id` and inline label fields. Keeping it as the deprecated legacy message
  makes the migration explicit.
- "Regular constraint" is the wording already used in the Rust docs for
  `f(x) = 0` / `f(x) <= 0` constraints.
- `ScalarConstraint` and `GeneralConstraint` are less aligned with current code
  vocabulary.

## Shared Sub-messages

### `Provenance`

Rust `constraint::Provenance` records special-constraint origins when a special
constraint is converted or propagated into regular constraints.

```proto
// proto/ommx/v1/provenance.proto
message Provenance {
  oneof source {
    uint64 indicator_constraint_id = 1;
    uint64 one_hot_constraint_id = 2;
    uint64 sos1_constraint_id = 3;
  }
}
```

This is not a general "absorbed legacy regular constraint ID" mechanism. The
v2 `ConstraintHints` migration context policy is described separately below.

### `RemovedReason`

Rust `constraint::RemovedReason` is currently stored alongside removed entries
in `ConstraintCollection<T>`.

```proto
// proto/ommx/v1/removed_reason.proto
message RemovedReason {
  string reason = 1;
  map<string, string> parameters = 2;
}
```

### `ModelingLabel`

Rust `ModelingLabel` is the common label shape for decision variables, named
functions, and the label component of constraint contexts.

```proto
// proto/ommx/v1/modeling_label.proto
message ModelingLabel {
  optional string name = 1;
  repeated int64 subscripts = 2;
  map<string, string> parameters = 3;
  optional string description = 4;
}
```

### `ConstraintContext`

Rust `ConstraintContext` groups the modeling label for a constraint together
with constraint-transformation provenance. This is the per-id transfer object
for `ConstraintContextStore<ID>`.

```proto
// proto/ommx/v1/constraint_context.proto
message ConstraintContext {
  ModelingLabel label = 1;
  repeated Provenance provenance = 2;
}
```

## Created-stage Messages

In the direct-map candidate, all created-stage messages use the same outer
shape:

- intrinsic fields,
- `ConstraintContext context`,
- `RemovedReason removed_reason`.

### `RegularConstraint`

```proto
// proto/ommx/v1/regular_constraint.proto
message RegularConstraint {
  Equality equality = 1;
  Function function = 2;
  ConstraintContext context = 3;
  RemovedReason removed_reason = 4;
}
```

### `IndicatorConstraint`

```proto
// proto/ommx/v1/indicator_constraint.proto
message IndicatorConstraint {
  // Binary decision variable that activates the constraint.
  uint64 indicator_variable = 1;
  Equality equality = 2;
  Function function = 3;
  ConstraintContext context = 4;
  RemovedReason removed_reason = 5;
}
```

### `OneHotConstraint`

```proto
// proto/ommx/v1/one_hot_constraint.proto
message OneHotConstraint {
  // Binary decision variables, exactly one of which must be 1.
  repeated uint64 decision_variables = 1;
  ConstraintContext context = 2;
  RemovedReason removed_reason = 3;
}
```

### `Sos1Constraint`

```proto
// proto/ommx/v1/sos1_constraint.proto
message Sos1Constraint {
  // Decision variables, at most one of which can be non-zero.
  repeated uint64 decision_variables = 1;
  ConstraintContext context = 2;
  RemovedReason removed_reason = 3;
}
```

## Evaluated-stage Messages

In the direct-map candidate, evaluated-stage messages mirror the current
runtime stage data and carry context/removal sidecars inline.
Unlike the v2 `EvaluatedConstraint`, the regular evaluated message stores
`feasible` explicitly because Rust `EvaluatedData` owns it.

```proto
message EvaluatedRegularConstraint {
  Equality equality = 1;
  double evaluated_value = 2;
  bool feasible = 3;
  repeated uint64 used_decision_variable_ids = 4;
  optional double dual_variable = 5;
  ConstraintContext context = 6;
  RemovedReason removed_reason = 7;
}

message EvaluatedIndicatorConstraint {
  uint64 indicator_variable = 1;
  Equality equality = 2;
  double evaluated_value = 3;
  bool feasible = 4;
  // Whether the indicator variable was active (= 1) at evaluation time.
  bool indicator_active = 5;
  repeated uint64 used_decision_variable_ids = 6;
  ConstraintContext context = 7;
  RemovedReason removed_reason = 8;
}

message EvaluatedOneHotConstraint {
  repeated uint64 decision_variables = 1;
  bool feasible = 2;
  // Which variable was 1, if exactly one was. Unset if infeasible.
  optional uint64 active_variable = 3;
  repeated uint64 used_decision_variable_ids = 4;
  ConstraintContext context = 5;
  RemovedReason removed_reason = 6;
}

message EvaluatedSos1Constraint {
  repeated uint64 decision_variables = 1;
  bool feasible = 2;
  // Which variable was non-zero, if exactly one was. Unset if all-zero or infeasible.
  optional uint64 active_variable = 3;
  repeated uint64 used_decision_variable_ids = 4;
  ConstraintContext context = 5;
  RemovedReason removed_reason = 6;
}
```

## Sampled-stage Messages

In the direct-map candidate, sampled one-hot and SOS1 data need to distinguish:

- sample recorded and no active variable,
- sample not recorded.

Use a wrapper message for map values so `None` can be represented for a present
sample.

File placement note: `SampledValues` currently lives in
`proto/ommx/v1/sample_set.proto`. If sampled constraint messages move into the
new per-constraint proto files sketched below, `SampledValues` must move first
to a shared proto such as `sampled_values.proto`. Otherwise `sample_set.proto`
would need to import the new sampled constraint messages while those messages
also import `SampledValues` from `sample_set.proto`, creating a protobuf import
cycle.

```proto
message SampledActiveVariable {
  optional uint64 variable_id = 1;
}

message SampledRegularConstraint {
  Equality equality = 1;
  SampledValues evaluated_values = 2;
  map<uint64, bool> feasible = 3;
  repeated uint64 used_decision_variable_ids = 4;
  optional SampledValues dual_variables = 5;
  ConstraintContext context = 6;
  RemovedReason removed_reason = 7;
}

message SampledIndicatorConstraint {
  uint64 indicator_variable = 1;
  Equality equality = 2;
  SampledValues evaluated_values = 3;
  map<uint64, bool> feasible = 4;
  map<uint64, bool> indicator_active = 5;
  repeated uint64 used_decision_variable_ids = 6;
  ConstraintContext context = 7;
  RemovedReason removed_reason = 8;
}

message SampledOneHotConstraint {
  repeated uint64 decision_variables = 1;
  map<uint64, bool> feasible = 2;
  map<uint64, SampledActiveVariable> active_variable = 3;
  repeated uint64 used_decision_variable_ids = 4;
  ConstraintContext context = 5;
  RemovedReason removed_reason = 6;
}

message SampledSos1Constraint {
  repeated uint64 decision_variables = 1;
  map<uint64, bool> feasible = 2;
  map<uint64, SampledActiveVariable> active_variable = 3;
  repeated uint64 used_decision_variable_ids = 4;
  ConstraintContext context = 5;
  RemovedReason removed_reason = 6;
}
```

## Top-level Message Changes

The field numbers below are based on the currently assigned fields in `main`;
verify them against the target commit before editing proto files.

The snippets use the direct-map candidate shape. If the Rust SDK normalization
PR concludes that collection wrapper messages are the better representation,
the new top-level fields should keep the same ownership split but use wrapper
message types instead of raw `map<uint64, ...>` fields.

### `Instance` (`proto/ommx/v1/instance.proto`)

Current fields `1..11` and `100` are already used. New fields start at `12`.

```proto
message Instance {
  // Existing v2 fields. Deprecated, read only for format_version == 0.
  repeated Constraint constraints = 4 [deprecated = true];
  ConstraintHints constraint_hints = 7 [deprecated = true];
  repeated RemovedConstraint removed_constraints = 8 [deprecated = true];

  // Existing current-main field.
  map<string, string> annotations = 11;

  // New v3 collections.
  map<uint64, RegularConstraint> regular_constraints = 12;
  map<uint64, IndicatorConstraint> indicator_constraints = 13;
  map<uint64, OneHotConstraint> one_hot_constraints = 14;
  map<uint64, Sos1Constraint> sos1_constraints = 15;

  uint32 format_version = 100; // value bumped 0 -> 1
}
```

### `ParametricInstance` (`proto/ommx/v1/parametric_instance.proto`)

Current fields `1..11` and `100` are already used. Use the same numbering as
`Instance` for the new collections.

```proto
message ParametricInstance {
  repeated Constraint constraints = 5 [deprecated = true];
  ConstraintHints constraint_hints = 7 [deprecated = true];
  repeated RemovedConstraint removed_constraints = 8 [deprecated = true];

  map<string, string> annotations = 11;

  map<uint64, RegularConstraint> regular_constraints = 12;
  map<uint64, IndicatorConstraint> indicator_constraints = 13;
  map<uint64, OneHotConstraint> one_hot_constraints = 14;
  map<uint64, Sos1Constraint> sos1_constraints = 15;

  uint32 format_version = 100;
}
```

### `Solution` (`proto/ommx/v1/solution.proto`)

Current fields `1..13` and `100` are already used. New fields start at `14`.

```proto
message Solution {
  repeated EvaluatedConstraint evaluated_constraints = 4 [deprecated = true];

  // Existing current-main fields.
  repeated EvaluatedNamedFunction evaluated_named_functions = 11;
  ProcessMetadata metadata = 12;
  map<string, string> annotations = 13;

  map<uint64, EvaluatedRegularConstraint> evaluated_regular_constraints = 14;
  map<uint64, EvaluatedIndicatorConstraint> evaluated_indicator_constraints = 15;
  map<uint64, EvaluatedOneHotConstraint> evaluated_one_hot_constraints = 16;
  map<uint64, EvaluatedSos1Constraint> evaluated_sos1_constraints = 17;

  uint32 format_version = 100;
}
```

### `SampleSet` (`proto/ommx/v1/sample_set.proto`)

Current fields `1..10` and `100` are already used. New fields start at `11`.

```proto
message SampleSet {
  repeated SampledConstraint constraints = 3 [deprecated = true];

  // Existing current-main fields.
  repeated SampledNamedFunction named_functions = 8;
  ProcessMetadata metadata = 9;
  map<string, string> annotations = 10;

  map<uint64, SampledRegularConstraint> sampled_regular_constraints = 11;
  map<uint64, SampledIndicatorConstraint> sampled_indicator_constraints = 12;
  map<uint64, SampledOneHotConstraint> sampled_one_hot_constraints = 13;
  map<uint64, SampledSos1Constraint> sampled_sos1_constraints = 14;

  uint32 format_version = 100;
}
```

## File Layout

```text
proto/ommx/v1/
  provenance.proto                 # new
  removed_reason.proto             # new
  modeling_label.proto             # new
  constraint_context.proto          # new: ModelingLabel + Provenance for constraints
  sampled_values.proto             # new/shared: SampledValues
  regular_constraint.proto         # new: created/evaluated/sampled regular messages
  indicator_constraint.proto       # new: created/evaluated/sampled indicator messages
  one_hot_constraint.proto         # new: created/evaluated/sampled one-hot messages
  sos1_constraint.proto            # new: created/evaluated/sampled SOS1 messages

  constraint.proto                 # keep deprecated v2 messages
  constraint_hints.proto           # keep deprecated v2 hints
  one_hot.proto                    # keep deprecated v2 hint message
  sos1.proto                       # keep deprecated v2 hint message

  instance.proto                   # add v3 collection fields, deprecate v2 fields
  parametric_instance.proto        # add v3 collection fields, deprecate v2 fields
  solution.proto                   # add v3 collection fields, deprecate v2 field
  sample_set.proto                 # add v3 collection fields, deprecate v2 field
```

## Read/write Behavior

### Format version 0 input

For `Instance` and `ParametricInstance`:

1. Parse legacy `constraints` and `removed_constraints`.
2. Parse `constraint_hints`.
3. Convert one-hot and SOS1 hints to first-class collections.
4. Remove absorbed regular constraints from the regular collection.

Constraint-context policy for v2 hint conversion:

- One-hot: copy the modeling label component from the referenced legacy regular
  constraint (`constraint_id`) to the new one-hot entry.
- SOS1: copy the modeling label component from the legacy binary/cardinality
  constraint (`binary_constraint_id`) to the new SOS1 entry.
- Modeling labels from legacy big-M constraints are not merged automatically.
  They belong to the encoding details being absorbed, not to the semantic SOS1
  object.
- `Provenance` is not used to record absorbed legacy regular constraint IDs,
  because the current provenance domain records special-constraint origins of
  regular constraints, not the reverse migration path.

For `Solution` and `SampleSet`:

1. Parse legacy regular evaluated/sampled constraints.
2. Initialize evaluated/sampled special-constraint collections as empty, because
   v2 data has no way to represent them.

### Format version 1 input

For each top-level message:

1. Reject non-empty legacy constraint fields.
2. Parse the new collection fields.
3. Split each collection entry into active vs. removed according to the final
   wire shape: `removed_reason` presence for the direct-map shape, or the
   wrapper's active/removed partition for the wrapper shape.
4. Insert constraint contexts into the corresponding collection context stores.
5. Run the same invariant validation currently enforced by builders/parsers:
   defined variables, binary requirements for indicator/one-hot, non-empty SOS1,
   active/removed consistency, evaluated/sample ID consistency, and
   context IDs that refer only to owned entries.

### Format version 1 output

- Write only the new collection fields.
- Leave deprecated legacy fields empty.
- Set `format_version = CURRENT_FORMAT_VERSION`, after bumping
  `CURRENT_FORMAT_VERSION` from `0` to `1`.
- Remove the current `Instance` serialization `unimplemented!` guard for
  special constraints.
- Remove the current lossy `Solution` / `SampleSet` serialization behavior by
  writing all evaluated/sampled collections.

## Implementation Notes

This should land as a short PR series rather than one schema-and-runtime change.
The proposal PR is intentionally document-only; the implementation work starts
after the Rust collection model has been checked once more.

Recommended PR sequence:

1. Merge this proposal document as the current PR.
   - Do not edit protobuf files.
   - Do not regenerate `rust/ommx/src/ommx.v1.rs`.
   - Treat the wire snippets above as candidate shapes, not as committed schema.
2. Create a Rust SDK normalization PR.
   - Verify that `ConstraintCollection<T>`, `EvaluatedCollection<T>`, and
     `SampledCollection<T>` have explicit ownership of their maps, removed
     reasons, and constraint-context stores.
   - Resolve [#958](https://github.com/Jij-Inc/ommx/issues/958) before the
     protobuf schema PR, so broader root-owned entity normalization is decided
     before the new wire shape is finalized.
   - Remove or narrow public raw mutation that can desynchronize sidecars, in
     particular the `DerefMut` exposure on `EvaluatedCollection<T>`.
   - Make `ConstraintCollection::insert_with` enforce the documented
     no-overwrite precondition, either by returning `Result` or by asserting at
     the crate-internal boundary.
   - Add validation helpers or tests for sidecar consistency:
     `removed_reasons` and constraint-context keys must refer to existing
     entries.
   - Consider common `ModelingLabelStore` / `ConstraintContextStore` helpers
     only if they clarify the ownership model; avoid merging independent
     constraint families into one enum-based collection.
   - Decide whether the protobuf representation should be the direct-map shape
     sketched here or a collection wrapper that mirrors the Rust owner more
     closely.
3. Create the protobuf schema PR.
   - Move `SampledValues` out of `sample_set.proto` into a shared proto before
     splitting sampled constraint messages across new constraint proto files.
   - Add proto messages and regenerate `rust/ommx/src/ommx.v1.rs`.
   - Implement v3 parse/write for `Instance` and `ParametricInstance`.
   - Implement v3 parse/write for `Solution` and `SampleSet`.
   - Keep protobuf serialization support at the top-level object boundary; do
     not add public standalone protobuf round-trip APIs for leaf values such as
     `Function`.
   - Keep format-0 legacy parsing tests and add format-1 round-trip tests for:
     - regular active and removed constraints,
     - indicator active and removed constraints,
     - one-hot active and removed constraints,
     - SOS1 active and removed constraints,
     - modeling-label / constraint-context / provenance round-trips,
     - evaluated and sampled special constraints.
   - Bump `CURRENT_FORMAT_VERSION` to `1`.
   - Regenerate Python stubs/docs artifacts if PyO3-visible signatures or
     generated exports move.
4. After the PR series lands, remove this proposal document.
   - Move stable API semantics into Rust/Python API Reference text.
   - Use the current label/context public wording in Python docs and examples:
     `variable_labels_df`, `constraint_context_df`, `constraint_provenance_df`,
     and `include="label"` rather than legacy `metadata` names.
   - Move wire-format semantics into protobuf comments.
   - Keep migration behavior in tests and migration/user-guide documentation,
     not in this temporary proposal file.

## Open Questions

1. Whether the final v3 wire shape should use the direct-map candidate shown in
   this document or collection wrapper messages that mirror the Rust owner more
   closely. The Rust SDK normalization PR should decide this before proto files
   are changed.
2. Whether to reject or ignore non-empty legacy constraint fields when
   `format_version == 1`. This proposal recommends rejection to avoid duplicate
   sources of truth.
3. Whether `SampledActiveVariable` should remain shared between OneHot and SOS1
   or be split into per-type wrappers. Shared matches the identical runtime
   shape and keeps the wire format smaller.
4. Whether v2 SOS1 hint migration should preserve any big-M constraint modeling
   labels outside the copied `binary_constraint_id` label. This proposal
   recommends not merging it, but implementation tests should make the chosen
   behavior explicit.
5. Solver adapter migration is outside the proto change itself, but adapters
   that still assume `instance.constraints` is the entire constraint universe
   need to use `required_capabilities` / `reduce_capabilities` or consume the
   special collections directly.
6. Broader root-owned entity normalization is tracked in
   [#958](https://github.com/Jij-Inc/ommx/issues/958), including inline IDs on
   variables/named functions/parameters, parameter labeling, and SampleSet's
   sample-ID universe.
