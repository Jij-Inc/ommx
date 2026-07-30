import OMMXProof.Instance.Transform
import OMMXProof.Instance.Transform.SOS1BigM.Target
import Mathlib.Tactic

/-!
# SOS1 Big-M lowering as an `Instance.Transform`

This module writes the SDK SOS1 conversion as an actual `Instance.Transform`,
which consists of a target instance, encode/decode map between the source and target state spaces.

## Lowering rule
Binary members are reused as their own selectors, and one fresh binary component is appended for every other member.

For example, `x ∈ Binary, y ∈ [0, 2], SOS1(x, y)` is lowered to `x, z ∈ Binary, y ∈ [0, 2], x + z ≤ 1, y ≤ 2z`.
No additional selector is appended for `x`. When `x = y = 0`, both `z = 0`
and `z = 1` are feasible in the target instance. Thus the
`decode: (x, y, z) ↦ (x, y)` map is not injective, and the yielded
`Instance.Transform` is not `TargetRoundTrip`.

`lowering` is partial: it returns `none` when a required member bound is not
finite or another plan precondition fails.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/-- Construct the SOS1 Big-M lowering when every required source bound is
finite. The state maps of a successfully constructed transform are total. -/
def lowering {source : Instance n} (plan : Plan source) :
    Option (Instance.Transform source) :=
  plan.validate.map fun validated =>
    { targetDimension := n + plan.freshCount
      target := validated.target
      encode := fun state =>
        some (State.append state fun j =>
          canonicalSelector (plan.memberState state) (plan.freshMember j))
      decode := fun state => some (State.source state) }

@[simp]
theorem lowering_isSome_iff_valid {source : Instance n} (plan : Plan source) :
    (lowering plan).isSome ↔ plan.Valid := by
  simp [lowering, Plan.validate]

@[simp]
theorem lowering_eq_none_iff_not_valid {source : Instance n}
    (plan : Plan source) :
    lowering plan = none ↔ ¬plan.Valid := by
  simp [lowering, Plan.validate]

theorem lowering_isReduction {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.IsReduction := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨validated, _, rfl⟩
  intro targetState htarget
  change validated.target.Feasible targetState at htarget
  refine ⟨State.source targetState, rfl, ?_⟩
  rcases
      (validated.target_feasible_iff_base_and_formulation targetState).mp
        htarget with
    ⟨hbase, hformulation⟩
  apply (plan.source_feasible_iff_base_and_selected
    (State.source targetState)).mpr
  exact ⟨hbase,
    validated.selectedHolds_of_plannedSelectorFormulation
      hbase.1 hformulation⟩

theorem lowering_isRelaxation {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.IsRelaxation := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨validated, _, rfl⟩
  intro sourceState hsource
  let selectors : State plan.freshCount :=
    fun j =>
      canonicalSelector (plan.memberState sourceState) (plan.freshMember j)
  refine ⟨State.append sourceState selectors, rfl, ?_⟩
  change validated.target.Feasible (State.append sourceState selectors)
  apply (validated.target_feasible_append_iff_base_and_formulation
    sourceState selectors).mpr
  rcases (plan.source_feasible_iff_base_and_selected sourceState).mp
      hsource with
    ⟨hbase, hselected⟩
  refine ⟨hbase, ?_⟩
  have hselectors :
      plan.freshSelectorState sourceState selectors =
        canonicalSelector (plan.memberState sourceState) := by
    funext i
    by_cases hi : i ∈ plan.freshMembers
    · simp [Plan.freshSelectorState, selectors, hi]
    · simp [Plan.freshSelectorState, hi]
  rw [hselectors]
  exact canonicalSelector_plannedFormulation
    plan.reusedMembers validated.bounds (plan.memberState sourceState)
    (validated.withinBounds_of_domains hbase.1)
    (plan.reusedBinary_of_domains hbase.1)
    ((plan.genericSOS1_memberState_iff_holds sourceState).mpr hselected)

theorem lowering_sensePreserving {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SensePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  rfl

theorem lowering_sourceObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SourceObjectiveValuePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro sourceState _
  simp [Instance.ObjectiveValue, Plan.Validated.target]

theorem lowering_targetObjectiveValuePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.TargetObjectiveValuePreserving := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro targetState _
  simp [Instance.ObjectiveValue, Plan.Validated.target]

theorem lowering_sourceObjectivePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SourceObjectivePreserving :=
  ⟨lowering_sensePreserving plan hlowering,
    lowering_sourceObjectiveValuePreserving plan hlowering⟩

theorem lowering_targetObjectivePreserving {source : Instance n}
    (plan : Plan source) {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.TargetObjectivePreserving :=
  ⟨lowering_sensePreserving plan hlowering,
    lowering_targetObjectiveValuePreserving plan hlowering⟩

theorem lowering_sourceRoundTrip {source : Instance n} (plan : Plan source)
    {transform : Instance.Transform source}
    (hlowering : lowering plan = some transform) :
    transform.SourceRoundTrip := by
  unfold lowering at hlowering
  rcases Option.map_eq_some_iff.mp hlowering with ⟨_, _, rfl⟩
  intro sourceState _
  simp

end SOS1BigM

end Instance

end OMMXProof
