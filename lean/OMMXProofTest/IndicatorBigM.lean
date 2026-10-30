import OMMXProof.Instance.Transform.IndicatorBigM

/-!
# Indicator Big-M transformation fixtures

The exact row-level fixture exercises omission of the redundant lower side.
Finite `Instance.Transform` fixtures are constructed through validated plans.
-/

namespace OMMXProof.Test.IndicatorBigM

open Instance.IndicatorBigM

def sdkBase (state : State 2) : Prop :=
  state 1 ∈ Domain.binary ∧
    0 ≤ state 0 ∧ state 0 ≤ 3

def sdkBody (state : State 2) : Rat := state 0

example (state : State 2) :
    (sdkBase state ∧
      (UpperSide sdkBody 1 3 state ∧
        LowerSide sdkBody 1 0 state)) ↔
      (sdkBase state ∧
        IndicatorPredicate 1 .activeOnOne
          (fun x => sdkBody x = 0) state) := by
  apply and_congr_right
  intro hbase
  exact equalitySides_iff_indicator
    hbase.1 hbase.2.1 hbase.2.2

example (state : State 2) :
    LowerSide sdkBody 1 0 state := by
  simp [LowerSide]

def domains : Fin 2 → Domain :=
  fun i =>
    if i = 0 then .continuous (.finite 0 3 (by norm_num))
    else .binary

def bodyExpr : Affine 2 where
  coeff := fun i => if i = 0 then 1 else 0
  constant := 0

def body : LinearConstraint 2 where
  expr := bodyExpr
  sense := .equal

def selected : IndicatorConstraint 2 where
  trigger := 1
  polarity := .activeOnOne
  body := body

def objective : Affine 2 where
  coeff := fun i => if i = 0 then 1 else 2
  constant := 0

def source : Instance 2 where
  domains := domains
  constraints := []
  indicatorConstraints := [selected]
  objective := objective
  sense := .minimize

def plan : Plan source where
  constraintIndex := ⟨0, by native_decide⟩

@[simp]
theorem plan_bodyValue (state : State 2) :
    plan.bodyValue state = state 0 := by
  simp [Plan.bodyValue, Plan.constraint, plan, source, selected, body,
    bodyExpr, Affine.eval]

example : plan.bodyBound = .finite 0 3 (by norm_num) := by
  native_decide

theorem plan_valid : plan.Valid := by native_decide

theorem plan_validate_isSome : plan.validate.isSome = true := by
  native_decide

def validated : plan.Validated :=
  plan.validate.get plan_validate_isSome

example : validated.upper = 3 := by native_decide

example : validated.lower (by native_decide) = 0 := by native_decide

theorem lowering_plan_isSome : (lowering plan).isSome = true := by
  native_decide

def transform : Instance.Transform source :=
  (lowering plan).get lowering_plan_isSome

theorem lowering_plan : lowering plan = some transform :=
  (Option.some_get lowering_plan_isSome).symm

/-- The positive upper side is emitted; the zero lower side is redundant. -/
example : validated.generatedConstraints.length = 1 := by native_decide

example : transform.target.constraints.length = 1 := by native_decide

example : transform.target.indicatorConstraints = [] := by native_decide

example : transform.targetDimension = 2 := by native_decide

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

example : transform.TargetRoundTrip :=
  lowering_targetRoundTrip plan lowering_plan

def upperOnlyDomains : Fin 2 → Domain :=
  fun i =>
    if i = 0 then .continuous (.upperBounded 3)
    else .binary

def lessEqualBody : LinearConstraint 2 :=
  { body with sense := .lessEqual }

def upperOnlyLessEqualSelected : IndicatorConstraint 2 :=
  { selected with body := lessEqualBody }

def upperOnlyLessEqualSource : Instance 2 :=
  { source with
    domains := upperOnlyDomains
    indicatorConstraints := [upperOnlyLessEqualSelected] }

def upperOnlyLessEqualPlan : Plan upperOnlyLessEqualSource where
  constraintIndex := ⟨0, by native_decide⟩

/-- A `≤` Indicator needs only the finite upper endpoint. -/
theorem upperOnlyLessEqualPlan_valid :
    upperOnlyLessEqualPlan.Valid := by
  native_decide

example : (lowering upperOnlyLessEqualPlan).isSome = true := by
  native_decide

def upperOnlyEqualitySource : Instance 2 :=
  { source with domains := upperOnlyDomains }

def upperOnlyEqualityPlan : Plan upperOnlyEqualitySource where
  constraintIndex := ⟨0, by native_decide⟩

/-- Equality lowering also needs a finite lower endpoint. -/
theorem upperOnlyEqualityPlan_invalid :
    ¬upperOnlyEqualityPlan.Valid := by
  native_decide

example : lowering upperOnlyEqualityPlan = none := by
  native_decide

def unboundedDomains : Fin 2 → Domain :=
  fun i => if i = 0 then .continuous else .binary

def unboundedLessEqualSource : Instance 2 :=
  { upperOnlyLessEqualSource with domains := unboundedDomains }

def unboundedLessEqualPlan : Plan unboundedLessEqualSource where
  constraintIndex := ⟨0, by native_decide⟩

/-- A `≤` Indicator still needs a finite upper endpoint. -/
theorem unboundedLessEqualPlan_invalid :
    ¬unboundedLessEqualPlan.Valid := by
  native_decide

example : lowering unboundedLessEqualPlan = none := by
  native_decide

def activeOnZeroSelected : IndicatorConstraint 2 :=
  { selected with polarity := .activeOnZero }

def activeOnZeroSource : Instance 2 :=
  { source with indicatorConstraints := [activeOnZeroSelected] }

def activeOnZeroPlan : Plan activeOnZeroSource where
  constraintIndex := ⟨0, by native_decide⟩

/-- The current generated rows encode an active-on-one Indicator only. -/
theorem activeOnZeroPlan_invalid : ¬activeOnZeroPlan.Valid := by
  native_decide

example : lowering activeOnZeroPlan = none := by
  native_decide

def nonBinarySource : Instance 2 :=
  { source with
    domains := fun _ => .continuous (.finite 0 3 (by norm_num)) }

def nonBinaryPlan : Plan nonBinarySource where
  constraintIndex := ⟨0, by native_decide⟩

/-- A nonbinary trigger cannot validate the Big-M lowering. -/
theorem nonBinaryPlan_invalid : ¬nonBinaryPlan.Valid := by
  native_decide

example : lowering nonBinaryPlan = none := by
  native_decide

end OMMXProof.Test.IndicatorBigM
