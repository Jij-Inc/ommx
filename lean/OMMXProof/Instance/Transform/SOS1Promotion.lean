import OMMXProof.Instance.Transform
import OMMXProof.Instance.Transform.SOS1Promotion.Target

/-!
# SOS1 promotion as an `Instance.Transform`

SOS1 promotion recognizes a standard selector formulation in a flat source
Instance and replaces it with one first-class SOS1 constraint.  In the initial
supported layout, retained variables and regular constraints occupy prefixes,
while fresh selectors and their link/cardinality rows occupy suffixes.

The transform itself is total for every untrusted `Witness`:

- `encode` projects a flat source state to its retained prefix;
- `decode` appends canonical selectors to a promoted target state.

Semantic properties are established separately from a validated
`StandardBigMForm`.  Thus rejection by the conservative validator means only
that this initial implementation does not recognize the source shape.
-/

namespace OMMXProof

namespace Instance

namespace SOS1Promotion

open SOS1SelectorFormulation

namespace Witness

/-- Promote the selector formulation described by an untrusted witness.

Construction is unconditional.  `validate` checks the sufficient conditions
used by the reduction, relaxation, and objective-preservation theorems. -/
def promotion (witness : Witness n)
    (source : Instance (n + witness.freshCount)) :
    Instance.Transform source where
  targetDimension := n
  target := witness.target source
  encode := fun state => some (State.source state)
  decode := fun state =>
    some (State.append state fun j =>
      canonicalSelector (witness.memberState state) (witness.freshMember j))

end Witness

/-- A feasible promoted state has a feasible canonical flat representation.

Target feasibility supplies the retained constraints and the promoted SOS1
condition.  Canonical selectors realize the validated link/cardinality suffix,
so appending them yields a feasible flat source state. -/
theorem promotion_isReduction {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (witness.promotion source).IsReduction := by
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
    (witness.promotion source).IsRelaxation := by
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
    (witness.promotion source).SensePreserving := by
  rfl

/-- Projecting a feasible flat state preserves its objective value.

The validator requires the source objective to be independent of the fresh
selector suffix. -/
theorem promotion_sourceObjectiveValuePreserving
    {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (witness.promotion source).SourceObjectiveValuePreserving := by
  intro flatState _
  simp only [Witness.promotion, Option.map_some, Option.some.injEq]
  simpa only [State.append_source_fresh] using
    (Witness.Validated.sourceObjectiveValue_append_eq_target
      (witness := witness) (source := source) validated
      (State.source flatState) (State.fresh flatState)).symm

/-- Canonically decoding a feasible promoted state preserves its objective. -/
theorem promotion_targetObjectiveValuePreserving
    {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (witness.promotion source).TargetObjectiveValuePreserving := by
  intro promotedState _
  change State n at promotedState
  let selectors : State witness.freshCount :=
    fun j =>
      canonicalSelector (witness.memberState promotedState)
        (witness.freshMember j)
  simp only [Witness.promotion, Option.map_some, Option.some.injEq]
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
    (witness.promotion source).SourceObjectivePreserving :=
  ⟨promotion_sensePreserving witness source,
    promotion_sourceObjectiveValuePreserving validated⟩

/-- Promotion preserves both optimization sense and objective value when
viewed from feasible promoted target states. -/
theorem promotion_targetObjectivePreserving
    {witness : Witness n}
    {source : Instance (n + witness.freshCount)}
    (validated : witness.Validated source) :
    (witness.promotion source).TargetObjectivePreserving :=
  ⟨promotion_sensePreserving witness source,
    promotion_targetObjectiveValuePreserving validated⟩

/-- Decoding a promoted state and encoding it again recovers that state.

This round trip is unconditional because `decode` only appends a suffix and
`encode` immediately projects back to the retained prefix. -/
theorem promotion_targetRoundTrip (witness : Witness n)
    (source : Instance (n + witness.freshCount)) :
    (witness.promotion source).TargetRoundTrip := by
  intro targetState _
  simp [Witness.promotion]

end SOS1Promotion

end Instance

end OMMXProof
