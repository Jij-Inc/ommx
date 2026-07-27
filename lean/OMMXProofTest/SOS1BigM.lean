import OMMXProof.Instance.Transform.SOS1BigM

/-!
# SOS1 Big-M transformation fixtures

The fixture mixes one reused binary member with one continuous member that
needs a fresh selector. Its lower bound is zero, so lowering from the witness
emits only the upper link for that member.
-/

namespace OMMXProof.Test.SOS1BigM

open Instance.SOS1BigM

/-! Mixed selector formulation: member 0 is reused as its own binary selector,
while member 1 gets a fresh selector. Its lower bound is zero, so the lower
link is omitted by the lowering. -/

def plannedReusedExample : Finset (Fin 2) := {0}

def plannedBoundsExample : SelectorBounds (Fin 2) where
  lower := fun _ => 0
  upper := fun i => if i.val = 0 then 1 else 3

def zeroExcludingFreshBoundsExample : SelectorBounds (Fin 2) where
  lower := fun i => if i.val = 0 then 0 else 1
  upper := fun i => if i.val = 0 then 1 else 3

example : ¬FreshBoundsContainZero plannedReusedExample
    zeroExcludingFreshBoundsExample := by
  native_decide

def plannedMembersExample : Fin 2 → Rat := fun i => if i.val = 0 then 0 else 2

def plannedFreshSelectorsExample : Fin 2 → Rat := fun _ => 1

example : PlannedSelectorFormulation plannedReusedExample plannedBoundsExample
    plannedMembersExample plannedFreshSelectorsExample := by
  native_decide

def invalidPlannedMembersExample : Fin 2 → Rat :=
  fun i => if i.val = 0 then 1 else 2

example : ¬PlannedSelectorFormulation plannedReusedExample plannedBoundsExample
    invalidPlannedMembersExample plannedFreshSelectorsExample := by
  native_decide

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

example : (lowering witness).targetDimension = 3 := by native_decide

example : (lowering witness).IsReduction :=
  lowering_isReduction witness witness_valid

example : (lowering witness).IsRelaxation :=
  lowering_isRelaxation witness witness_valid

example : (lowering witness).SensePreserving :=
  lowering_sensePreserving witness

example : (lowering witness).SourceObjectiveValuePreserving :=
  lowering_sourceObjectiveValuePreserving witness

example : (lowering witness).TargetObjectiveValuePreserving :=
  lowering_targetObjectiveValuePreserving witness

example : (lowering witness).SourceObjectivePreserving :=
  lowering_sourceObjectivePreserving witness

example : (lowering witness).TargetObjectivePreserving :=
  lowering_targetObjectivePreserving witness

example : (lowering witness).SourceRoundTrip :=
  lowering_sourceRoundTrip witness

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
    witness.target.ObjectiveValue
        (State.append zeroSource fun j =>
          canonicalSelector
            (witness.memberState zeroSource) (witness.freshMember j)) =
      source.ObjectiveValue zeroSource := by
  simpa [lowering] using
    lowering_sourceObjectiveValuePreserving witness zeroSource_feasible

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
    ¬(lowering witness).TargetRoundTrip := by
  intro hroundTrip
  have hstate := hroundTrip noncanonicalTarget_feasible
  change
    some (State.append (State.source noncanonicalTarget) fun j =>
      canonicalSelector
        (witness.memberState (State.source noncanonicalTarget))
        (witness.freshMember j)) =
      some noncanonicalTarget at hstate
  have heq :
      State.append (State.source noncanonicalTarget) (fun j =>
        canonicalSelector
          (witness.memberState (State.source noncanonicalTarget))
          (witness.freshMember j)) =
        noncanonicalTarget :=
    Option.some.inj hstate
  have hcomponent := congrArg
    (fun state => state (Fin.natAdd 2 freshZero)) heq
  simp [Witness.memberState, noncanonicalTarget, oneSelector, zeroSource,
    canonicalSelector, State.source, State.append] at hcomponent

end OMMXProof.Test.SOS1BigM
