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

namespace Witness

def decodeState {source : Instance n} (witness : Witness source)
    (state : State (n + witness.freshCount)) : State n :=
  State.source state

def encodeSelectors {source : Instance n} (witness : Witness source)
    (state : State n) : State witness.freshCount :=
  fun j => canonicalSelector (witness.memberState state) (witness.freshMember j)

def encodeState {source : Instance n} (witness : Witness source)
    (state : State n) : State (n + witness.freshCount) :=
  State.append state (witness.encodeSelectors state)

@[simp]
theorem decode_encode {source : Instance n} (witness : Witness source)
    (state : State n) :
    witness.decodeState (witness.encodeState state) = state := by
  simp [decodeState, encodeState]

/-- The SOS1 Big-M lowering packaged in the general Instance transformation
shape. The state maps are total; `Option` is introduced by `Transform`. -/
def lowering {source : Instance n} (witness : Witness source) :
    Instance.Transform source where
  targetDimension := n + witness.freshCount
  target := witness.target
  encode := fun state => some (witness.encodeState state)
  decode := fun state => some (witness.decodeState state)

theorem lowering_isReduction {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) :
    witness.lowering.IsReduction := by
  intro targetState htarget
  refine ⟨witness.decodeState targetState, rfl, ?_⟩
  simpa [decodeState] using
    witness.source_feasible_of_target_feasible hvalid htarget

theorem lowering_isRelaxation {source : Instance n} (witness : Witness source)
    (hvalid : witness.Valid) :
    witness.lowering.IsRelaxation := by
  intro sourceState hsource
  refine ⟨witness.encodeState sourceState, rfl, ?_⟩
  change witness.target.Feasible
    (State.append sourceState
      (fun j =>
        canonicalSelector
          (witness.memberState sourceState) (witness.freshMember j)))
  exact witness.target_feasible_append_canonical hvalid hsource

theorem lowering_sensePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.SensePreserving :=
  rfl

theorem lowering_sourceObjectiveValuePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.SourceObjectiveValuePreserving := by
  intro sourceState _
  simp [lowering, Instance.ObjectiveValue, target, encodeState]

theorem lowering_targetObjectiveValuePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.TargetObjectiveValuePreserving := by
  intro targetState _
  simp [lowering, Instance.ObjectiveValue, target, decodeState]

theorem lowering_sourceObjectivePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.SourceObjectivePreserving :=
  ⟨witness.lowering_sensePreserving,
    witness.lowering_sourceObjectiveValuePreserving⟩

theorem lowering_targetObjectivePreserving {source : Instance n}
    (witness : Witness source) :
    witness.lowering.TargetObjectivePreserving :=
  ⟨witness.lowering_sensePreserving,
    witness.lowering_targetObjectiveValuePreserving⟩

theorem lowering_sourceRoundTrip {source : Instance n}
    (witness : Witness source) :
    witness.lowering.SourceRoundTrip := by
  intro sourceState _
  simp [lowering, witness.decode_encode]

end Witness

end SOS1BigM

end Instance

end OMMXProof
