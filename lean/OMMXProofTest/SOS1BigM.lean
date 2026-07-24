import OMMXProof.Instance.Transform.SOS1BigM

/-!
# SOS1 Big-M transformation fixtures

The fixture mixes one reused binary member with one continuous member that
needs a fresh selector. Its lower bound is zero, so lowering from the witness
emits only the upper link for that member.
-/

namespace OMMXProof.Test.SOS1BigM

open Instance.SOS1BigM

def members : Finset (Fin 2) := Finset.univ

def domains : Fin 2 → Domain :=
  fun i =>
    if i = 0 then .binary
    else .continuous (.finite 0 2 (by norm_num))

def objective : Affine 2 where
  coeff := fun i => if i = 0 then 1 else 2
  constant := 0

def selected : SOS1Constraint 2 where
  members := members

def source : Instance 2 where
  domains := domains
  constraints := []
  sos1Constraints := [selected]
  objective := objective
  sense := .minimize

def witness : Witness source where
  constraintIndex := ⟨0, by native_decide⟩
  bounds :=
    { lower := fun _ => 0
      upper := fun i => if i.1 = 0 then 1 else 2 }

theorem witness_valid : witness.Valid := by native_decide

example : witness.reusedMembers.card = 1 := by native_decide

example : witness.freshMembers.card = 1 := by native_decide

example : witness.freshCount = 1 := by native_decide

/-- One nontrivial upper link; the zero lower-bound side is omitted. -/
example : witness.linkConstraints.length = 1 := by native_decide

example : witness.generatedConstraints.length = 2 := by native_decide

example : witness.lowering.targetDimension = 3 := by native_decide

example : witness.lowering.IsReduction :=
  witness.lowering_isReduction witness_valid

example : witness.lowering.IsRelaxation :=
  witness.lowering_isRelaxation witness_valid

example : witness.lowering.SensePreserving :=
  witness.lowering_sensePreserving

example : witness.lowering.SourceObjectiveValuePreserving :=
  witness.lowering_sourceObjectiveValuePreserving

example : witness.lowering.TargetObjectiveValuePreserving :=
  witness.lowering_targetObjectiveValuePreserving

example : witness.lowering.SourceObjectivePreserving :=
  witness.lowering_sourceObjectivePreserving

example : witness.lowering.TargetObjectivePreserving :=
  witness.lowering_targetObjectivePreserving

example : witness.lowering.SourceRoundTrip :=
  witness.lowering_sourceRoundTrip

def unboundedSource : Instance 1 where
  domains := fun _ => .continuous
  constraints := []
  sos1Constraints := [{ members := Finset.univ }]
  objective := Affine.zero
  sense := .minimize

def unboundedWitness : Witness unboundedSource where
  constraintIndex := ⟨0, by native_decide⟩
  bounds := ⟨fun _ => 0, fun _ => 0⟩

/-- A valid witness for a fresh selector requires finite source bounds. -/
example : ¬unboundedWitness.Valid := by native_decide

def zeroSource : State 2 := fun _ => 0

theorem zeroSource_feasible : source.Feasible zeroSource := by
  unfold Instance.Feasible
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · intro i
    fin_cases i <;> native_decide
  · simp [source]
  · simp [source]
  · simp [source, selected, SOS1Constraint.Holds, zeroSource]
  · simp [source]

example :
    witness.target.ObjectiveValue (witness.encodeState zeroSource) =
      source.ObjectiveValue zeroSource := by
  simpa [Witness.lowering] using
    witness.lowering_sourceObjectiveValuePreserving zeroSource_feasible

def oneSelector : State witness.freshCount := fun _ => 1

def noncanonicalTarget : State (2 + witness.freshCount) :=
  State.append zeroSource oneSelector

theorem noncanonicalTarget_feasible :
    witness.target.Feasible noncanonicalTarget := by
  rw [noncanonicalTarget,
    witness.target_feasible_append_iff_base_and_formulation]
  refine ⟨(witness.source_feasible_iff_base_and_selected zeroSource).mp
    zeroSource_feasible |>.1, ?_⟩
  native_decide

def freshZero : Fin witness.freshCount :=
  ⟨0, by native_decide⟩

/-- A zero member permits either selector value, so canonical re-encoding does
not recover every feasible target state. -/
theorem not_targetRoundTrip :
    ¬witness.lowering.TargetRoundTrip := by
  intro hroundTrip
  have hstate := hroundTrip noncanonicalTarget_feasible
  change some (witness.encodeState (witness.decodeState noncanonicalTarget)) =
    some noncanonicalTarget at hstate
  have heq :
      witness.encodeState (witness.decodeState noncanonicalTarget) =
        noncanonicalTarget :=
    Option.some.inj hstate
  have hcomponent := congrArg
    (fun state => state (Fin.natAdd 2 freshZero)) heq
  simp [Witness.encodeState, Witness.decodeState, Witness.encodeSelectors,
    Witness.memberState, noncanonicalTarget, oneSelector, zeroSource,
    canonicalSelector, State.source, State.append] at hcomponent

end OMMXProof.Test.SOS1BigM
