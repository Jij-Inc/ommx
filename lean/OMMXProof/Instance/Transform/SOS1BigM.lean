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
No additional variable for `x` is not appended. For `x = y = 0` case, both `z = 0` and `z = 1` are feasible
in the target instance. So the `decode: (x, y, z) ↦ (x, y)` map is not injective.
This means the yielded `Instance.Transform` is not `TargetRoundTrip`.
-/

namespace OMMXProof

namespace Instance

namespace SOS1BigM

/-- The SOS1 Big-M lowering packaged in the general Instance transformation
shape. The state maps are total; `Option` is introduced by `Transform`. -/
def lowering {source : Instance n} (witness : Witness source) :
    Instance.Transform source where
  targetDimension := n + witness.freshCount
  target := witness.target
  encode := fun state =>
    some (State.append state fun j =>
      canonicalSelector (witness.memberState state) (witness.freshMember j))
  decode := fun state => some (State.source state)

theorem lowering_isReduction {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) :
    (lowering witness).IsReduction := by
  intro targetState htarget
  change witness.target.Feasible targetState at htarget
  refine ⟨State.source targetState, rfl, ?_⟩
  rcases (witness.target_feasible_iff_base_and_formulation targetState).mp
      htarget with ⟨hbase, hformulation⟩
  apply (witness.source_feasible_iff_base_and_selected
    (State.source targetState)).mpr
  exact ⟨hbase,
    witness.selectedHolds_of_plannedSelectorFormulation
      hvalid hbase.1 hformulation⟩

theorem lowering_isRelaxation {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) :
    (lowering witness).IsRelaxation := by
  intro sourceState hsource
  let selectors : State witness.freshCount :=
    fun j =>
      canonicalSelector (witness.memberState sourceState) (witness.freshMember j)
  refine ⟨State.append sourceState selectors, rfl, ?_⟩
  change witness.target.Feasible (State.append sourceState selectors)
  apply (witness.target_feasible_append_iff_base_and_formulation
    sourceState selectors).mpr
  rcases (witness.source_feasible_iff_base_and_selected sourceState).mp
      hsource with ⟨hbase, hselected⟩
  refine ⟨hbase, ?_⟩
  have hselectors :
      witness.freshSelectorState sourceState selectors =
        canonicalSelector (witness.memberState sourceState) := by
    funext i
    by_cases hi : i ∈ witness.freshMembers
    · simp [Witness.freshSelectorState, selectors, hi]
    · simp [Witness.freshSelectorState, hi]
  rw [hselectors]
  exact canonicalSelector_plannedFormulation
    witness.reusedMembers witness.bounds (witness.memberState sourceState)
    (witness.withinBounds_of_domains hvalid hbase.1)
    (witness.reusedBinary_of_domains hbase.1)
    ((witness.genericSOS1_memberState_iff_holds sourceState).mpr hselected)

theorem lowering_sensePreserving {source : Instance n}
    (witness : Witness source) :
    (lowering witness).SensePreserving :=
  rfl

theorem lowering_sourceObjectiveValuePreserving {source : Instance n}
    (witness : Witness source) :
    (lowering witness).SourceObjectiveValuePreserving := by
  intro sourceState _
  simp [lowering, Instance.ObjectiveValue, Witness.target]

theorem lowering_targetObjectiveValuePreserving {source : Instance n}
    (witness : Witness source) :
    (lowering witness).TargetObjectiveValuePreserving := by
  intro targetState _
  simp [lowering, Instance.ObjectiveValue, Witness.target]

theorem lowering_sourceObjectivePreserving {source : Instance n}
    (witness : Witness source) :
    (lowering witness).SourceObjectivePreserving :=
  ⟨lowering_sensePreserving witness,
    lowering_sourceObjectiveValuePreserving witness⟩

theorem lowering_targetObjectivePreserving {source : Instance n}
    (witness : Witness source) :
    (lowering witness).TargetObjectivePreserving :=
  ⟨lowering_sensePreserving witness,
    lowering_targetObjectiveValuePreserving witness⟩

theorem lowering_sourceRoundTrip {source : Instance n}
    (witness : Witness source) :
    (lowering witness).SourceRoundTrip := by
  intro sourceState _
  simp [lowering]

end SOS1BigM

end Instance

end OMMXProof
