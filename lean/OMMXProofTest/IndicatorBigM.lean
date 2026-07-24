import OMMXProof.Instance.Transform.IndicatorBigM

/-!
# Indicator Big-M transformation fixtures

The exact row-level fixture exercises omission of the redundant lower side.
Finite `Instance.Transform` fixtures are defined below it.
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
  bodyBound := .finite 0 3 (by norm_num)

@[simp]
theorem plan_bodyValue (state : State 2) :
    plan.bodyValue state = state 0 := by
  simp [Plan.bodyValue, Plan.constraint, plan, source, selected, body,
    bodyExpr, Affine.eval]

theorem plan_valid : plan.Valid := by native_decide

example :
    Plan.create source ⟨0, by native_decide⟩ = some plan := by
  native_decide

/-- The positive upper side is emitted; the zero lower side is redundant. -/
example : plan.generatedConstraints.length = 1 := by native_decide

example : plan.target.constraints.length = 1 := by native_decide

example : plan.target.indicatorConstraints = [] := by native_decide

example : plan.lowering.targetDimension = 2 := by native_decide

example : plan.lowering.IsReduction :=
  plan.lowering_isReduction plan_valid

example : plan.lowering.IsRelaxation :=
  plan.lowering_isRelaxation plan_valid

example : plan.lowering.SourceObjectivePreserving :=
  plan.lowering_sourceObjectivePreserving

example : plan.lowering.TargetObjectivePreserving :=
  plan.lowering_targetObjectivePreserving

example : plan.lowering.SourceRoundTrip :=
  plan.lowering_sourceRoundTrip

example : plan.lowering.TargetRoundTrip :=
  plan.lowering_targetRoundTrip

def unsafeUpperPlan : Plan source where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .finite 0 2 (by norm_num)

/-- A stored bound different from the computed affine image is rejected. -/
theorem unsafeUpperPlan_invalid : ¬unsafeUpperPlan.Valid := by
  native_decide

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
  bodyBound := .upperBounded 3

/-- A `≤` Indicator needs only the finite upper endpoint. -/
theorem upperOnlyLessEqualPlan_valid :
    upperOnlyLessEqualPlan.Valid := by
  native_decide

example :
    Plan.create upperOnlyLessEqualSource ⟨0, by native_decide⟩ =
      some upperOnlyLessEqualPlan := by
  native_decide

def upperOnlyEqualitySource : Instance 2 :=
  { source with domains := upperOnlyDomains }

def upperOnlyEqualityPlan : Plan upperOnlyEqualitySource where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .upperBounded 3

/-- Equality lowering also needs a finite lower endpoint. -/
theorem upperOnlyEqualityPlan_invalid :
    ¬upperOnlyEqualityPlan.Valid := by
  native_decide

example :
    Plan.create upperOnlyEqualitySource ⟨0, by native_decide⟩ = none := by
  native_decide

def activeOnZeroSelected : IndicatorConstraint 2 :=
  { selected with polarity := .activeOnZero }

def activeOnZeroSource : Instance 2 :=
  { source with indicatorConstraints := [activeOnZeroSelected] }

def activeOnZeroPlan : Plan activeOnZeroSource where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .finite 0 3 (by norm_num)

/-- The current generated rows encode an active-on-one Indicator only. -/
theorem activeOnZeroPlan_invalid : ¬activeOnZeroPlan.Valid := by
  intro hvalid
  have hne :
      activeOnZeroPlan.constraint.polarity ≠ .activeOnOne := by
    native_decide
  exact hne hvalid.2.1

def nonBinarySource : Instance 2 :=
  { source with
    domains := fun _ => .continuous (.finite 0 3 (by norm_num)) }

def nonBinaryPlan : Plan nonBinarySource where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .finite 0 3 (by norm_num)

/-- A nonbinary trigger cannot validate the Big-M lowering. -/
theorem nonBinaryPlan_invalid : ¬nonBinaryPlan.Valid := by
  intro hvalid
  have hne :
      nonBinarySource.domains nonBinaryPlan.constraint.trigger ≠
        .binary := by
    native_decide
  exact hne hvalid.1

end OMMXProof.Test.IndicatorBigM
