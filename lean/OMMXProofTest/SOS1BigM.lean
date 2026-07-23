import OMMXProof.Instance.Transform.SOS1BigM

/-!
# SOS1 Big-M transformation fixtures

The fixture mixes one reused binary member with one continuous member that
needs a fresh selector. Its lower bound is zero, so the SDK plan emits only
the upper link for that member.
-/

namespace OMMXProof.Test.SOS1BigM

open Instance.SOS1BigM

def members : Finset (Fin 2) := Finset.univ

def domains : Fin 2 → Domain :=
  fun i =>
    if i = 0 then .binary
    else .continuous (some 0) (some 2)

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

def plan : Plan source where
  constraintIndex := ⟨0, by native_decide⟩
  bounds :=
    { lower := fun _ => 0
      upper := fun i => if i.1 = 0 then 1 else 2 }

theorem plan_valid : plan.Valid := by native_decide

example : plan.reusedMembers.card = 1 := by native_decide

example : plan.freshMembers.card = 1 := by native_decide

example : plan.freshCount = 1 := by native_decide

/-- One nontrivial upper link; the zero lower-bound side is omitted. -/
example : plan.linkConstraints.length = 1 := by native_decide

example : plan.generatedConstraints.length = 2 := by native_decide

example : plan.lowering.targetDimension = 3 := by native_decide

example : plan.lowering.IsReduction :=
  plan.lowering_isReduction plan_valid

example : plan.lowering.IsRelaxation :=
  plan.lowering_isRelaxation plan_valid

example : plan.lowering.SourceRoundTrip :=
  plan.lowering_sourceRoundTrip

def unboundedSource : Instance 1 where
  domains := fun _ => .continuous
  constraints := []
  sos1Constraints := [{ members := Finset.univ }]
  objective := Affine.zero
  sense := .minimize

def unboundedPlan : Plan unboundedSource where
  constraintIndex := ⟨0, by native_decide⟩
  bounds := ⟨fun _ => 0, fun _ => 0⟩

/-- A fresh selector cannot be planned without finite source bounds. -/
example : ¬unboundedPlan.Valid := by native_decide

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
    plan.target.ObjectiveValue (plan.encodeState zeroSource) =
      source.ObjectiveValue zeroSource :=
  (plan.projectionPreserves plan_valid).objective_lift zeroSource_feasible

def oneSelector : State plan.freshCount := fun _ => 1

def noncanonicalTarget : State (2 + plan.freshCount) :=
  State.append zeroSource oneSelector

theorem noncanonicalTarget_feasible :
    plan.target.Feasible noncanonicalTarget := by
  rw [noncanonicalTarget,
    plan.target_feasible_append_iff_base_and_gadget]
  refine ⟨(plan.source_feasible_iff_base_and_selected zeroSource).mp
    zeroSource_feasible |>.1, ?_⟩
  native_decide

def freshZero : Fin plan.freshCount :=
  ⟨0, by native_decide⟩

/-- A zero member permits either selector value, so canonical re-encoding does
not recover every feasible target state. -/
theorem not_targetRoundTrip :
    ¬plan.lowering.TargetRoundTrip := by
  intro hroundTrip
  have hstate := hroundTrip noncanonicalTarget_feasible
  change some (plan.encodeState (plan.decodeState noncanonicalTarget)) =
    some noncanonicalTarget at hstate
  have heq :
      plan.encodeState (plan.decodeState noncanonicalTarget) =
        noncanonicalTarget :=
    Option.some.inj hstate
  have hcomponent := congrArg
    (fun state => state (Fin.natAdd 2 freshZero)) heq
  simp [Plan.encodeState, Plan.decodeState, Plan.encodeSelectors,
    Plan.memberState, noncanonicalTarget, oneSelector, zeroSource,
    canonicalSelector, State.source, State.append] at hcomponent

end OMMXProof.Test.SOS1BigM
