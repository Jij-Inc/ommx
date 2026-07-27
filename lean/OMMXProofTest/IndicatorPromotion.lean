import OMMXProof.Instance.Transform.IndicatorPromotion

/-!
# Indicator promotion transformation fixtures

The exact fixture retains `x - 2y ≤ 0` while promoting `x - y ≤ 0`.
For `y = 1`, the derived Indicator body is `x - 1 ≤ 0`. For `y = 0`,
the retained row implies the removed row, which supplies the external inactive
branch proof required by `Witness.Valid`.
-/

namespace OMMXProof.Test.IndicatorPromotion

open Instance.IndicatorPromotion

def domains : Fin 2 → Domain :=
  fun i =>
    if i = 0 then .continuous (.finite 0 2 (by norm_num))
    else .binary

def selectedExpr : Affine 2 where
  coeff := fun i => if i = 0 then 1 else -1
  constant := 0

def selected : LinearConstraint 2 where
  expr := selectedExpr
  sense := .lessEqual

def baseExpr : Affine 2 where
  coeff := fun i => if i = 0 then 1 else -2
  constant := 0

def base : LinearConstraint 2 where
  expr := baseExpr
  sense := .lessEqual

def objective : Affine 2 where
  coeff := fun i => if i = 0 then 1 else 3
  constant := 0

def source : Instance 2 where
  domains := domains
  constraints := [selected, base]
  objective := objective
  sense := .minimize

def witness : Witness source where
  constraintIndex := ⟨0, by native_decide⟩
  trigger := 1
  polarity := .activeOnOne

theorem witness_valid : witness.Valid := by
  constructor
  · native_decide
  · intro state hbase hinactive
    have hy : state 1 = 0 := by
      simpa [witness, IndicatorPolarity.inactiveValue] using hinactive
    have hsurviving : base.Holds state :=
      hbase.2.1 base (by simp [source, witness])
    simpa [Witness.constraint, witness, source, selected, selectedExpr,
      base, baseExpr, LinearConstraint.Holds, Affine.eval, hy] using
        hsurviving

example : witness.target.constraints = [base] := by rfl

example : witness.target.indicatorConstraints = [witness.indicator] := by rfl

example : witness.promotion.targetDimension = 2 := by rfl

example : witness.promotion.IsReduction :=
  witness.promotion_isReduction witness_valid

example : witness.promotion.IsRelaxation :=
  witness.promotion_isRelaxation witness_valid

example : witness.promotion.SourceObjectivePreserving :=
  witness.promotion_sourceObjectivePreserving

example : witness.promotion.TargetObjectivePreserving :=
  witness.promotion_targetObjectivePreserving

example : witness.promotion.SourceRoundTrip :=
  witness.promotion_sourceRoundTrip

example : witness.promotion.TargetRoundTrip :=
  witness.promotion_targetRoundTrip

/-! Without the inactive implication, the structurally valid Indicator can
allow target states which violate the removed source row. -/

def unsafeSource : Instance 2 where
  domains := domains
  constraints := [selected]
  objective := objective
  sense := .minimize

def unsafeWitness : Witness unsafeSource where
  constraintIndex := ⟨0, by native_decide⟩
  trigger := 1
  polarity := .activeOnOne

def inactiveCounterexample : State 2 :=
  fun i => if i = 0 then 1 else 0

example : unsafeSource.domains unsafeWitness.trigger = .binary := by
  native_decide

theorem inactiveCounterexample_target_feasible :
    unsafeWitness.target.Feasible inactiveCounterexample := by
  unfold Instance.Feasible
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · intro i
    fin_cases i <;> native_decide
  · simp [Witness.target, unsafeWitness, unsafeSource]
  · simp [Witness.target, unsafeWitness, unsafeSource]
  · simp [Witness.target, unsafeWitness, unsafeSource]
  · simp [Witness.target, Witness.indicator, Witness.constraint,
      unsafeWitness, unsafeSource, IndicatorConstraint.Holds,
      IndicatorPolarity.Active, IndicatorPolarity.activeValue,
      inactiveCounterexample]

theorem inactiveCounterexample_not_source_feasible :
    ¬unsafeSource.Feasible inactiveCounterexample := by
  intro h
  have hselected := h.2.1 selected (by simp [unsafeSource])
  norm_num [selected, selectedExpr, LinearConstraint.Holds, Affine.eval,
    inactiveCounterexample] at hselected

example : ¬unsafeWitness.promotion.IsReduction := by
  intro h
  rcases h inactiveCounterexample_target_feasible with
    ⟨sourceState, hdecode, hsource⟩
  change some inactiveCounterexample = some sourceState at hdecode
  have hstate : inactiveCounterexample = sourceState :=
    Option.some.inj hdecode
  subst sourceState
  exact inactiveCounterexample_not_source_feasible hsource

end OMMXProof.Test.IndicatorPromotion
