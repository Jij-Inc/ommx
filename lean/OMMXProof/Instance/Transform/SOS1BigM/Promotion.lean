import OMMXProof.Instance.Transform
import OMMXProof.Instance.Transform.SOS1BigM.Promotion.Validation

/-!
# SOS1 Big-M promotion as an `Instance.Transform`

SOS1 Big-M promotion recognizes a standard Big-M selector formulation in a
flat source Instance and replaces it with one first-class SOS1 constraint. In
the initial supported layout, retained variables and regular constraints
occupy prefixes, while fresh selectors and their link/cardinality rows occupy
suffixes. Existing special constraints are preserved when they do not
reference fresh selectors.

The transform itself is total for every untrusted `Witness`:

- `encode` projects a flat source state to its retained prefix;
- `decode` appends canonical selectors to a promoted target state.

Semantic properties are established separately from a validated
`StandardBigMForm`.  Thus rejection by the conservative validator means only
that this initial implementation does not recognize the source shape.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/--
# SOS1 Big-M promotion

## Promotion rule

- Retain the decision-variable and regular-constraint prefixes selected by the witness.
- Retain existing OneHot, SOS1, and Indicator constraints which do not use fresh selectors.
- Remove the fresh-selector suffix and its exact Big-M link/cardinality rows.
- Add one first-class SOS1 constraint over the witnessed members.
- `promotion` itself is total. The reduction, relaxation, and objective-preservation
  theorems require `witness.validate source` to succeed.

## Sufficient condition and executable check

The sufficient condition is the proposition
`witness.StandardBigMForm source`. It requires:

- finite declared bounds for every promoted SOS1 member;
- reused selectors to be exactly the retained binary members;
- every fresh selector in the source suffix to be binary;
- a valid retained-constraint prefix whose rows do not depend on fresh selectors;
- the remaining ordered regular-constraint suffix to equal the canonical Big-M
  link/cardinality rows generated from the witness and declared bounds;
- every existing OneHot and SOS1 constraint to contain only retained members;
- every existing Indicator constraint to have a retained trigger and a body independent
  of fresh selectors; and
- the objective to be independent of fresh selectors.

This proposition is decidable. `witness.validate source` is its executable checker and
returns the proposition bundled as `Witness.Validated`. The theorem
`Witness.validate_isSome_iff_standardBigMForm` states the exact checker contract:

```
(witness.validate source).isSome ↔ witness.StandardBigMForm source
```

Correspondingly, `Witness.validate_eq_none_iff_not_standardBigMForm` characterizes
rejection. The reduction, relaxation, and objective-preservation theorems consume the
returned `Witness.Validated`; rejection does not prove that promotion is semantically
incorrect outside this sufficient-condition recognizer.

## Example

For example, `x, z ∈ Binary, y ∈ [0, 2], x + z ≤ 1, y ≤ 2z` is promoted to
`x ∈ Binary, y ∈ [0, 2], SOS1(x, y)`.

This uses a `Witness 2` with the following data:

- The retained prefix has dimension `2`, with component `0` named `x` and component `1`
  named `y`.
- `members = {x, y}` selects both retained components for the promoted SOS1 constraint.
- `freshMembers = {y}` declares that `y` uses a fresh selector. Consequently `x` is the
  reused selector, `freshCount = 1`, and the appended source component is named `z`.
- `retainedConstraintCount = 0` declares that no regular row is retained. The entire
  ordered constraint list is therefore the generated suffix `[y ≤ 2z, x + z ≤ 1]`.

Under this witness:

- `y ≤ 2z` is recognized as the upper link obtained from the upper bound `2` and
  removed. No lower link is expected because the lower bound of `y` is zero.
- `x + z ≤ 1` is recognized as the selector-cardinality row and removed.
- `SOS1(x, y)` is added to the promoted target.
- `(x, y, z)` is the flat source state and `(x, y)` is the promoted target state.
- `encode` discards `z`: `(x, y, z) ↦ (x, y)`.
- `decode` restores the canonical selector,
  `z = if y = 0 then 0 else 1`.

When `x = y = 0`, both `z = 0` and `z = 1` are feasible in the flat source.
Encoding `(0, 0, 1)` and then decoding yields `(0, 0, 0)`, so the transform is not
`SourceRoundTrip`. Target round-trip does hold because decoding always chooses the canonical
selector and encoding immediately discards it.

## Validation failures

The untrusted witness always constructs a transform, but `witness.validate source` returns
`none` unless the source has the initial supported standard form. For example:

- Replacing `x + z ≤ 1` with `x + z ≤ 2` is rejected. This row does not enforce the
  at-most-one selector condition and can project to a state violating `SOS1(x, y)`.
- Replacing `y ≤ 2z` with `y ≤ 3z` is also rejected. Given `y ∈ [0, 2]`, this looser
  coefficient can still describe a valid formulation, but the initial checker deliberately
  requires the canonical row generated from the declared upper bound.
- Making an existing OneHot or SOS1 constraint contain `z`, or using `z` as an Indicator
  trigger or in its body, is rejected because that constraint cannot be preserved after
  removing `z`.
- Making the source objective depend on `z` is rejected because removing `z` would not
  preserve objective values.

Thus validation failure means that this conservative Big-M recognizer does not certify the
input, not that no mathematically correct SOS1 promotion exists.
-/
def promotion (witness : Witness n)
    (source : Instance (n + witness.freshCount)) :
    Instance.Transform source where
  targetDimension := n
  target := witness.target source
  encode := fun state => some (State.source state)
  decode := fun state =>
    some (State.append state fun j =>
      canonicalSelector (witness.memberState state) (witness.freshMember j))

/-- A feasible promoted state has a feasible canonical flat representation.

Target feasibility supplies the retained constraints and the promoted SOS1
condition.  Canonical selectors realize the validated link/cardinality suffix,
so appending them yields a feasible flat source state. -/
theorem promotion_isReduction {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (promotion witness source).IsReduction := by
  intro promotedState htarget
  change State n at promotedState
  change (witness.target source).Feasible promotedState at htarget
  let selectors : State witness.freshCount :=
    fun j =>
      canonicalSelector (witness.memberState promotedState)
        (witness.freshMember j)
  refine ⟨State.append promotedState selectors, rfl, ?_⟩
  apply
    (Witness.Validated.source_feasible_append_iff_base_and_formulation
      (witness := witness) (source := source) validated
      promotedState selectors).mpr
  rcases
      (witness.target_feasible_iff_base_and_selected
        source promotedState).mp htarget with
    ⟨hbase, hselected⟩
  refine ⟨hbase, ?_⟩
  have hselectors :
      witness.freshSelectorState promotedState selectors =
        canonicalSelector (witness.memberState promotedState) := by
    funext i
    by_cases hi : i ∈ witness.freshMembers
    · simp [Witness.freshSelectorState, selectors, hi]
    · simp [Witness.freshSelectorState, hi]
  rw [hselectors]
  exact
    Witness.Validated.canonicalSelectorFormulation_of_selected
      (witness := witness) (source := source) validated
      hbase.1 hselected

/-- Projecting a feasible flat state yields a feasible promoted SOS1 state.

Validated source feasibility exposes the selector formulation.  Projecting
that formulation proves the first-class SOS1 condition, while the prefix
retains every domain and regular-constraint fact in the promoted target. -/
theorem promotion_isRelaxation {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (promotion witness source).IsRelaxation := by
  intro flatState hsource
  refine ⟨State.source flatState, rfl, ?_⟩
  apply
    (witness.target_feasible_iff_base_and_selected
      source (State.source flatState)).mpr
  rcases
      (Witness.Validated.source_feasible_iff_base_and_formulation
        (witness := witness) (source := source) validated flatState).mp
          hsource with
    ⟨hbase, hformulation⟩
  exact
    ⟨hbase,
      Witness.Validated.selectedHolds_of_plannedSelectorFormulation
        (witness := witness) (source := source) validated
        hbase.1 hformulation⟩

/-- Promotion preserves the optimization sense by construction. -/
theorem promotion_sensePreserving (witness : Witness n)
    (source : Instance (n + witness.freshCount)) :
    (promotion witness source).SensePreserving := by
  rfl

/-- Projecting a feasible flat state preserves its objective value.

The validator requires the source objective to be independent of the fresh
selector suffix. -/
theorem promotion_sourceObjectiveValuePreserving
    {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (promotion witness source).SourceObjectiveValuePreserving := by
  intro flatState _
  simp only [promotion, Option.map_some, Option.some.injEq]
  simpa only [State.append_source_fresh] using
    (Witness.Validated.sourceObjectiveValue_append_eq_target
      (witness := witness) (source := source) validated
      (State.source flatState) (State.fresh flatState)).symm

/-- Canonically decoding a feasible promoted state preserves its objective. -/
theorem promotion_targetObjectiveValuePreserving
    {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (promotion witness source).TargetObjectiveValuePreserving := by
  intro promotedState _
  change State n at promotedState
  let selectors : State witness.freshCount :=
    fun j =>
      canonicalSelector (witness.memberState promotedState)
        (witness.freshMember j)
  simp only [promotion, Option.map_some, Option.some.injEq]
  change
    source.ObjectiveValue (State.append promotedState selectors) =
      (witness.target source).ObjectiveValue promotedState
  exact
    Witness.Validated.sourceObjectiveValue_append_eq_target
      (witness := witness) (source := source) validated
      promotedState selectors

/-- Promotion preserves both optimization sense and objective value when
viewed from feasible flat source states. -/
theorem promotion_sourceObjectivePreserving
    {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (promotion witness source).SourceObjectivePreserving :=
  ⟨promotion_sensePreserving witness source,
    promotion_sourceObjectiveValuePreserving validated⟩

/-- Promotion preserves both optimization sense and objective value when
viewed from feasible promoted target states. -/
theorem promotion_targetObjectivePreserving
    {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (promotion witness source).TargetObjectivePreserving :=
  ⟨promotion_sensePreserving witness source,
    promotion_targetObjectiveValuePreserving validated⟩

/-- Decoding a promoted state and encoding it again recovers that state.

This round trip is unconditional because `decode` only appends a suffix and
`encode` immediately projects back to the retained prefix. -/
theorem promotion_targetRoundTrip (witness : Witness n)
    (source : Instance (n + witness.freshCount)) :
    (promotion witness source).TargetRoundTrip := by
  intro targetState _
  simp [promotion]

end SOS1BigM

end Instance

end OMMXProof
