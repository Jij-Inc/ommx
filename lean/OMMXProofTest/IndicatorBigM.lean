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

def witness : Witness source where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .finite 0 3 (by norm_num)

@[simp]
theorem witness_bodyValue (state : State 2) :
    witness.bodyValue state = state 0 := by
  simp [Witness.bodyValue, Witness.constraint, witness, source, selected, body,
    bodyExpr, Affine.eval]

theorem witness_valid : witness.Valid := by native_decide

example :
    Witness.create source ⟨0, by native_decide⟩ = some witness := by
  native_decide

/-- The positive upper side is emitted; the zero lower side is redundant. -/
example : witness.generatedConstraints.length = 1 := by native_decide

example : witness.target.constraints.length = 1 := by native_decide

example : witness.target.indicatorConstraints = [] := by native_decide

example : witness.lowering.targetDimension = 2 := by native_decide

example : witness.lowering.IsReduction :=
  witness.lowering_isReduction witness_valid

example : witness.lowering.IsRelaxation :=
  witness.lowering_isRelaxation witness_valid

example : witness.lowering.SourceObjectivePreserving :=
  witness.lowering_sourceObjectivePreserving

example : witness.lowering.TargetObjectivePreserving :=
  witness.lowering_targetObjectivePreserving

example : witness.lowering.SourceRoundTrip :=
  witness.lowering_sourceRoundTrip

example : witness.lowering.TargetRoundTrip :=
  witness.lowering_targetRoundTrip

def unsafeUpperWitness : Witness source where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .finite 0 2 (by norm_num)

/-- A stored bound different from the computed affine image is rejected. -/
theorem unsafeUpperWitness_invalid : ¬unsafeUpperWitness.Valid := by
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

def upperOnlyLessEqualWitness : Witness upperOnlyLessEqualSource where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .upperBounded 3

/-- A `≤` Indicator needs only the finite upper endpoint. -/
theorem upperOnlyLessEqualWitness_valid :
    upperOnlyLessEqualWitness.Valid := by
  native_decide

example :
    Witness.create upperOnlyLessEqualSource ⟨0, by native_decide⟩ =
      some upperOnlyLessEqualWitness := by
  native_decide

def upperOnlyEqualitySource : Instance 2 :=
  { source with domains := upperOnlyDomains }

def upperOnlyEqualityWitness : Witness upperOnlyEqualitySource where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .upperBounded 3

/-- Equality lowering also needs a finite lower endpoint. -/
theorem upperOnlyEqualityWitness_invalid :
    ¬upperOnlyEqualityWitness.Valid := by
  native_decide

example :
    Witness.create upperOnlyEqualitySource ⟨0, by native_decide⟩ = none := by
  native_decide

def activeOnZeroSelected : IndicatorConstraint 2 :=
  { selected with polarity := .activeOnZero }

def activeOnZeroSource : Instance 2 :=
  { source with indicatorConstraints := [activeOnZeroSelected] }

def activeOnZeroWitness : Witness activeOnZeroSource where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .finite 0 3 (by norm_num)

/-- The current generated rows encode an active-on-one Indicator only. -/
theorem activeOnZeroWitness_invalid : ¬activeOnZeroWitness.Valid := by
  intro hvalid
  have hne :
      activeOnZeroWitness.constraint.polarity ≠ .activeOnOne := by
    native_decide
  exact hne hvalid.2.1

def nonBinarySource : Instance 2 :=
  { source with
    domains := fun _ => .continuous (.finite 0 3 (by norm_num)) }

def nonBinaryWitness : Witness nonBinarySource where
  constraintIndex := ⟨0, by native_decide⟩
  bodyBound := .finite 0 3 (by norm_num)

/-- A nonbinary trigger cannot validate the Big-M lowering. -/
theorem nonBinaryWitness_invalid : ¬nonBinaryWitness.Valid := by
  intro hvalid
  have hne :
      nonBinarySource.domains nonBinaryWitness.constraint.trigger ≠
        .binary := by
    native_decide
  exact hne hvalid.1

end OMMXProof.Test.IndicatorBigM
