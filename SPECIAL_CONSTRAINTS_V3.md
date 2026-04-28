# Special Constraints in OMMX v3 — Proto Schema Proposal

Status: **Draft / WIP**

## Goal

Replace the v2 `ConstraintHints`-based representation of special constraints
(OneHot, SOS1, ...) with first-class proto messages, mirroring the Rust SDK v3
runtime structure where each special-constraint type has its own
`ConstraintCollection` keyed by a dedicated ID.

In the same break, unify the regular `Constraint` representation with the new
special-constraint shape:

- ID owned by the enclosing collection (`map<uint64, T>`), not by the message.
- Active/removed distinction encoded inline via `optional RemovedReason`,
  not by separate fields on the parent message.
- `Provenance` chain serialized.

## Compatibility policy

- `format_version` is bumped from `0` to `1` in the same change.
- Forward compatibility is **deliberately broken**: a v2 SDK reading v3 data
  hits the `format_version` check and refuses to load.
- Backward compatibility is preserved on the v3 read side: when
  `format_version == 0`, v3 reads from the old fields
  (`constraints`, `removed_constraints`, `constraint_hints`,
  `evaluated_constraints`, ...) and translates `ConstraintHints` into the new
  first-class collections (the existing `convert_hints_to_collections` path
  is retained).
- v3 writers always emit the new fields and never the old ones.

## Naming used below

- "Created" / "Evaluated" / "Sampled" refer to the lifecycle stages the Rust SDK
  v3 uses (`stage::Created`, `stage::Evaluated`, `stage::Sampled`).
- "Regular" is the working name for the renamed v3 form of the existing
  `Constraint` (alternatives: `Scalar`, `General`). See open questions.

## Shared sub-messages (new)

### `Provenance`

`rust/ommx/src/constraint.rs::Provenance` is an enum over the three special-
constraint ID types. One-to-one mapped to a proto `oneof`:

```proto
// proto/ommx/v1/provenance.proto
message Provenance {
  oneof source {
    uint64 indicator_constraint_id = 1;
    uint64 one_hot_constraint_id   = 2;
    uint64 sos1_constraint_id      = 3;
  }
}
```

### `RemovedReason`

`rust/ommx/src/constraint/stage.rs::RemovedReason`. In v2 these two fields were
inlined onto `RemovedConstraint` / `EvaluatedConstraint` / `SampledConstraint`;
in v3 they are wrapped:

```proto
// proto/ommx/v1/removed_reason.proto
message RemovedReason {
  string reason = 1;
  map<string, string> parameters = 2;
}
```

### `ConstraintMetadata`

`rust/ommx/src/constraint.rs::ConstraintMetadata`, extracted as a sub-message
so all four constraint types can embed it once instead of inlining the same
five fields:

```proto
// proto/ommx/v1/constraint_metadata.proto
message ConstraintMetadata {
  optional string name = 1;
  repeated int64 subscripts = 2;
  map<string, string> parameters = 3;
  optional string description = 4;
  repeated Provenance provenance = 5;
}
```

## Created stage messages (new)

All four use the same shape: type-specific fields + `ConstraintMetadata` +
`optional RemovedReason`. The presence of `removed_reason` is the only marker
distinguishing active vs. removed constraints inside the same map.

### `RegularConstraint`

```proto
// proto/ommx/v1/regular_constraint.proto
message RegularConstraint {
  Equality equality = 1;
  Function function = 2;
  ConstraintMetadata metadata = 3;
  optional RemovedReason removed_reason = 4;
}
```

### `OneHotConstraint`

```proto
// proto/ommx/v1/one_hot_constraint.proto
message OneHotConstraint {
  // Binary decision variables, exactly one of which must be 1.
  repeated uint64 decision_variables = 1;
  ConstraintMetadata metadata = 2;
  optional RemovedReason removed_reason = 3;
}
```

### `Sos1Constraint`

```proto
// proto/ommx/v1/sos1_constraint.proto
message Sos1Constraint {
  // Decision variables, at most one of which can be non-zero.
  repeated uint64 decision_variables = 1;
  ConstraintMetadata metadata = 2;
  optional RemovedReason removed_reason = 3;
}
```

### `IndicatorConstraint`

```proto
// proto/ommx/v1/indicator_constraint.proto
message IndicatorConstraint {
  uint64 indicator_variable = 1;
  Equality equality = 2;
  Function function = 3;
  ConstraintMetadata metadata = 4;
  optional RemovedReason removed_reason = 5;
}
```

## Evaluated stage messages (new)

Same shape as Created with type-specific evaluation fields appended:

```proto
message EvaluatedRegularConstraint {
  Equality equality = 1;
  double evaluated_value = 2;
  bool feasible = 3;
  repeated uint64 used_decision_variable_ids = 4;
  optional double dual_variable = 5;
  ConstraintMetadata metadata = 6;
  optional RemovedReason removed_reason = 7;
}

message EvaluatedOneHotConstraint {
  repeated uint64 decision_variables = 1;
  bool feasible = 2;
  // Which variable was 1, if exactly one was. Unset if infeasible.
  optional uint64 active_variable = 3;
  repeated uint64 used_decision_variable_ids = 4;
  ConstraintMetadata metadata = 5;
  optional RemovedReason removed_reason = 6;
}

message EvaluatedSos1Constraint {
  repeated uint64 decision_variables = 1;
  bool feasible = 2;
  // Which variable was non-zero, if exactly one was. Unset if all-zero or infeasible.
  optional uint64 active_variable = 3;
  repeated uint64 used_decision_variable_ids = 4;
  ConstraintMetadata metadata = 5;
  optional RemovedReason removed_reason = 6;
}

message EvaluatedIndicatorConstraint {
  uint64 indicator_variable = 1;
  Equality equality = 2;
  double evaluated_value = 3;
  bool feasible = 4;
  // Whether the indicator variable was active (= 1) at evaluation time.
  bool indicator_active = 5;
  repeated uint64 used_decision_variable_ids = 6;
  ConstraintMetadata metadata = 7;
  optional RemovedReason removed_reason = 8;
}
```

Note on `removed_reason` in evaluated forms: in v2 it was carried by
`EvaluatedConstraint` directly via inline `string removed_reason` +
`map<string, string> removed_reason_parameters`. In v3 it is unified to
`optional RemovedReason`.

## Sampled stage messages (new)

Maps from `sample_id` to per-sample data. The Rust runtime carries
`BTreeMap<SampleID, T>` plus `Option<T>`-valued maps for
`active_variable`. The `Option<T>` is encoded via a wrapper message so that
"sample recorded but no active variable" is distinguishable from "sample not
recorded":

```proto
// active_variable wrapper for OneHot / SOS1 sampled forms
message SampledActiveVariable {
  optional uint64 variable_id = 1;
}

message SampledRegularConstraint {
  Equality equality = 1;
  // evaluated value per sample
  SampledValues evaluated_values = 2;
  // feasibility per sample
  map<uint64, bool> feasible = 3;
  repeated uint64 used_decision_variable_ids = 4;
  optional SampledValues dual_variables = 5;
  ConstraintMetadata metadata = 6;
  optional RemovedReason removed_reason = 7;
}

message SampledOneHotConstraint {
  repeated uint64 decision_variables = 1;
  map<uint64, bool> feasible = 2;
  map<uint64, SampledActiveVariable> active_variable = 3;
  repeated uint64 used_decision_variable_ids = 4;
  ConstraintMetadata metadata = 5;
  optional RemovedReason removed_reason = 6;
}

message SampledSos1Constraint {
  repeated uint64 decision_variables = 1;
  map<uint64, bool> feasible = 2;
  map<uint64, SampledActiveVariable> active_variable = 3;
  repeated uint64 used_decision_variable_ids = 4;
  ConstraintMetadata metadata = 5;
  optional RemovedReason removed_reason = 6;
}

message SampledIndicatorConstraint {
  uint64 indicator_variable = 1;
  Equality equality = 2;
  SampledValues evaluated_values = 3;
  map<uint64, bool> feasible = 4;
  map<uint64, bool> indicator_active = 5;
  repeated uint64 used_decision_variable_ids = 6;
  ConstraintMetadata metadata = 7;
  optional RemovedReason removed_reason = 8;
}
```

## Top-level message changes

### `Instance` (`proto/ommx/v1/instance.proto`)

```proto
message Instance {
  // ----- Existing v2 fields (deprecated; read-only on v3 input when format_version == 0) -----
  repeated Constraint constraints = 4 [deprecated = true];
  ConstraintHints constraint_hints = 7 [deprecated = true];
  repeated RemovedConstraint removed_constraints = 8 [deprecated = true];

  // ----- New v3 collections -----
  map<uint64, RegularConstraint>    regular_constraints   = 11;
  map<uint64, OneHotConstraint>     one_hot_constraints   = 12;
  map<uint64, Sos1Constraint>       sos1_constraints      = 13;
  map<uint64, IndicatorConstraint>  indicator_constraints = 14;

  // ----- Unchanged: description, decision_variables, objective, sense, parameters,
  //                  decision_variable_dependency, named_functions, format_version -----

  uint32 format_version = 100;  // value bumped 0 -> 1
}
```

### `ParametricInstance` (`proto/ommx/v1/parametric_instance.proto`)

Same treatment as `Instance`: deprecate `constraints` (5),
`constraint_hints` (7), `removed_constraints` (8), and add the four new
`map<uint64, *>` fields starting from the next available number.

### `Solution` (`proto/ommx/v1/solution.proto`)

```proto
message Solution {
  // ----- Existing v2 -----
  repeated EvaluatedConstraint evaluated_constraints = 4 [deprecated = true];

  // ----- New v3 -----
  map<uint64, EvaluatedRegularConstraint>    evaluated_regular_constraints   = 12;
  map<uint64, EvaluatedOneHotConstraint>     evaluated_one_hot_constraints   = 13;
  map<uint64, EvaluatedSos1Constraint>       evaluated_sos1_constraints      = 14;
  map<uint64, EvaluatedIndicatorConstraint>  evaluated_indicator_constraints = 15;
  // Other fields unchanged.
}
```

### `SampleSet` (`proto/ommx/v1/sample_set.proto`)

```proto
message SampleSet {
  // ----- Existing v2 -----
  repeated SampledConstraint constraints = 3 [deprecated = true];

  // ----- New v3 -----
  map<uint64, SampledRegularConstraint>    sampled_regular_constraints   = 11;
  map<uint64, SampledOneHotConstraint>     sampled_one_hot_constraints   = 12;
  map<uint64, SampledSos1Constraint>       sampled_sos1_constraints      = 13;
  map<uint64, SampledIndicatorConstraint>  sampled_indicator_constraints = 14;
  // Other fields unchanged.
}
```

## File layout

```
proto/ommx/v1/
  provenance.proto                ← new
  removed_reason.proto            ← new
  constraint_metadata.proto       ← new
  regular_constraint.proto        ← new (3 stages)
  one_hot_constraint.proto        ← new (3 stages)
  sos1_constraint.proto           ← new (3 stages)
  indicator_constraint.proto      ← new (3 stages)

  constraint.proto                ← unchanged (deprecated v2 messages stay)
  constraint_hints.proto          ← unchanged (deprecated)
  one_hot.proto                   ← unchanged (deprecated v2 hint message)
  sos1.proto                      ← unchanged (deprecated v2 hint message)

  instance.proto                  ← add new fields, deprecate old
  parametric_instance.proto       ← add new fields, deprecate old
  solution.proto                  ← add new fields, deprecate old
  sample_set.proto                ← add new fields, deprecate old
```

## `format_version` bump

`rust/ommx/src/lib.rs::CURRENT_FORMAT_VERSION`: `0 → 1`.

Writer side (`From<Instance> for v1::Instance`, …) already references the
constant, so the value flips automatically. Old fields are written as the
default empty value.

Reader side: existing `check_format_version` accepts `<= CURRENT_FORMAT_VERSION`,
so both v2 (`0`) and v3 (`1`) data load. The Rust parse path branches on
`format_version` to decide whether to consult old fields or new fields.

## Open questions

1. **Naming of the renamed regular constraint** — `RegularConstraint`
   (working name), `ScalarConstraint`, `GeneralConstraint`, or keep using the
   name `Constraint` and rename the v2 message to e.g. `LegacyConstraint`?
2. **`ConstraintHints` deprecation marker** — file-level comment only, or also
   `[deprecated = true]` on every field of `ConstraintHints` / `OneHot` / `SOS1`?
3. **`SampledActiveVariable` reuse vs. specialization** — one shared wrapper for
   OneHot and SOS1, or separate `SampledOneHotActive` / `SampledSos1Active`
   types? Shared is simpler; separate keeps each special-constraint .proto
   self-contained.
4. **Migration of solver adapters** — out of scope for the proto change itself,
   but worth noting that adapters touching `instance.constraints` directly will
   need to switch to the new map.
