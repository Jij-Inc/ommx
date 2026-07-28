import OMMXProof.Instance.Transform.SOS1BigM

/-!
# SOS1 Big-M transformation fixtures

The fixture mixes one reused binary member with one continuous member that
needs a fresh selector. Its lower bound is zero, so lowering from the plan
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

def plan : Plan source where
  constraintIndex := ⟨0, by native_decide⟩

theorem plan_valid : plan.Valid := by native_decide

theorem plan_validate_isSome : plan.validate.isSome = true := by
  native_decide

def validated : plan.Validated :=
  plan.validate.get plan_validate_isSome

theorem lowering_plan_isSome : (lowering plan).isSome = true := by
  native_decide

def transform : Instance.Transform source :=
  (lowering plan).get lowering_plan_isSome

theorem lowering_plan : lowering plan = some transform :=
  (Option.some_get lowering_plan_isSome).symm

example : plan.reusedMembers.card = 1 := by native_decide

example : plan.freshMembers.card = 1 := by native_decide

example : plan.freshCount = 1 := by native_decide

def freshZero : Fin plan.freshCount :=
  ⟨0, by native_decide⟩

example : validated.bounds.lower (plan.freshMember freshZero) = 0 := by
  native_decide

example : validated.bounds.upper (plan.freshMember freshZero) = 2 := by
  native_decide

/-- One nontrivial upper link; the zero lower-bound side is omitted. -/
example : validated.linkConstraints.length = 1 := by native_decide

example : validated.generatedConstraints.length = 2 := by native_decide

example : transform.targetDimension = 3 := by native_decide

example : transform.IsReduction :=
  lowering_isReduction plan lowering_plan

example : transform.IsRelaxation :=
  lowering_isRelaxation plan lowering_plan

example : transform.SensePreserving :=
  lowering_sensePreserving plan lowering_plan

example : transform.SourceObjectiveValuePreserving :=
  lowering_sourceObjectiveValuePreserving plan lowering_plan

example : transform.TargetObjectiveValuePreserving :=
  lowering_targetObjectiveValuePreserving plan lowering_plan

example : transform.SourceObjectivePreserving :=
  lowering_sourceObjectivePreserving plan lowering_plan

example : transform.TargetObjectivePreserving :=
  lowering_targetObjectivePreserving plan lowering_plan

example : transform.SourceRoundTrip :=
  lowering_sourceRoundTrip plan lowering_plan

def unboundedSource : Instance 1 where
  domains := fun _ => .continuous
  constraints := []
  sos1Constraints := [{ members := Finset.univ }]
  objective := Affine.zero
  sense := .minimize

def unboundedPlan : Plan unboundedSource where
  constraintIndex := ⟨0, by native_decide⟩

/-- A valid plan for a fresh selector requires finite source bounds. -/
example : ¬unboundedPlan.Valid := by native_decide

example : lowering unboundedPlan = none := by native_decide

def lowerBoundedSource : Instance 1 :=
  { unboundedSource with
    domains := fun _ => .continuous (.lowerBounded 0) }

def lowerBoundedPlan : Plan lowerBoundedSource where
  constraintIndex := ⟨0, by native_decide⟩

example : ¬lowerBoundedPlan.Valid := by native_decide

example : lowering lowerBoundedPlan = none := by native_decide

def upperBoundedSource : Instance 1 :=
  { unboundedSource with
    domains := fun _ => .continuous (.upperBounded 0) }

def upperBoundedPlan : Plan upperBoundedSource where
  constraintIndex := ⟨0, by native_decide⟩

example : ¬upperBoundedPlan.Valid := by native_decide

example : lowering upperBoundedPlan = none := by native_decide

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
    Option.map transform.target.ObjectiveValue
        (transform.encode zeroSource) =
      some (source.ObjectiveValue zeroSource) :=
  lowering_sourceObjectiveValuePreserving plan lowering_plan
    zeroSource_feasible

def oneSelector : State plan.freshCount := fun _ => 1

def noncanonicalTarget : State (2 + plan.freshCount) :=
  State.append zeroSource oneSelector

/-- A zero member permits either selector value, so canonical re-encoding does
not recover every feasible target state. -/
theorem not_targetRoundTrip_of_lowering
    {lowered : Instance.Transform source}
    (hlowering : lowering plan = some lowered) :
    ¬lowered.TargetRoundTrip := by
  unfold lowering at hlowering
  cases hvalidated : plan.validate with
  | none =>
      simp [hvalidated] at hlowering
  | some validated =>
      simp only [hvalidated, Option.map_some, Option.some.injEq] at hlowering
      subst lowered
      have htarget :
          validated.target.Feasible noncanonicalTarget := by
        rw [noncanonicalTarget,
          validated.target_feasible_append_iff_base_and_formulation]
        refine
          ⟨(validated.source_feasible_iff_base_and_selected zeroSource).mp
              zeroSource_feasible |>.1,
            ?_⟩
        native_decide +revert
      intro hroundTrip
      have hstate := hroundTrip htarget
      change
        some (State.append (State.source noncanonicalTarget) fun j =>
          canonicalSelector
            (plan.memberState (State.source noncanonicalTarget))
            (plan.freshMember j)) =
          some noncanonicalTarget at hstate
      have heq :
          State.append (State.source noncanonicalTarget) (fun j =>
            canonicalSelector
              (plan.memberState (State.source noncanonicalTarget))
              (plan.freshMember j)) =
            noncanonicalTarget :=
        Option.some.inj hstate
      have hcomponent := congrArg
        (fun state => state (Fin.natAdd 2 freshZero)) heq
      simp [Plan.memberState, noncanonicalTarget, oneSelector, zeroSource,
        canonicalSelector, State.source, State.append] at hcomponent

example : ¬transform.TargetRoundTrip :=
  not_targetRoundTrip_of_lowering lowering_plan

end OMMXProof.Test.SOS1BigM
