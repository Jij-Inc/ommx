import OMMXProof.Constraint.OneHot
import OMMXProof.Constraint.SOS1.Instance

/-!
# Executable test fixtures and counterexamples

Every accepted checker example has a nearby rejected adversarial variant. The
fixtures are isolated from the soundness library and elaborated by `lake test`.
-/

namespace OMMXProof.Test.Fixtures

def oneVarAffine (coefficient constant : Rat) : Affine 1 where
  coeff := fun _ => coefficient
  constant := constant

/-- Regression fixture for the affine denotation: the constant is added once,
not once per coordinate. -/
def constantOnlyAffine2 : Affine 2 where
  coeff := fun _ => 0
  constant := 1

example : constantOnlyAffine2.eval (fun _ => 0) = 1 := by native_decide

example (lhs rhs : Affine 1) (sense : ConstraintSense)
    (state : State 1) :
    (LinearConstraint.normalize lhs rhs sense).Holds state ↔
      match sense with
      | .lessEqual => lhs.eval state ≤ rhs.eval state
      | .equal => lhs.eval state = rhs.eval state :=
  LinearConstraint.normalize_holds_iff lhs rhs sense state

def binaryDomain : Domain := .binary

def binaryDomains2 : Fin 2 → Domain := fun _ => binaryDomain

def allTwo : Finset (Fin 2) := Finset.univ

def scaledOneHotSource : LinearConstraint 2 :=
  { expr := Affine.scale (-2) (oneHotExpr allTwo), sense := .equal }

def oneHotDraft : OneHotDraft 2 := { members := allTwo, scale := -2 }

example : checkOneHot binaryDomains2 scaledOneHotSource oneHotDraft = true := by
  native_decide

def emptyOneHotDraft : OneHotDraft 2 := { members := ∅, scale := 1 }

example : checkOneHot binaryDomains2
    { expr := oneHotExpr ∅, sense := .equal } emptyOneHotDraft = false := by
  native_decide

def continuousDomains2 : Fin 2 → Domain := fun _ =>
  .continuous

example : checkOneHot continuousDomains2 scaledOneHotSource oneHotDraft = false := by
  native_decide

/-- The binary-domain premise is essential: `(2, -1)` satisfies the structural
sum equation but is not a OneHot state. -/
theorem continuous_sum_is_not_oneHot :
    let state : State 2 := fun i => if i.val = 0 then 2 else -1
    (oneHotExpr allTwo).eval state = 0 ∧
      ¬({ members := allTwo } : OneHotConstraint 2).Holds state := by
  dsimp
  constructor
  · native_decide
  · native_decide

def mismatchedOneHotSource : LinearConstraint 2 where
  expr :=
    { coeff := fun i => if i.val = 0 then 1 else 2
      constant := -1 }
  sense := .equal

example : checkOneHot binaryDomains2 mismatchedOneHotSource
    { members := allTwo, scale := 1 } = false := by native_decide

def wrongSenseOneHotSource : LinearConstraint 2 :=
  { expr := oneHotExpr allTwo, sense := .lessEqual }

example : checkOneHot binaryDomains2 wrongSenseOneHotSource
    { members := allTwo, scale := 1 } = false := by native_decide

/-- Merely matching the affine expression is insufficient: the zero state
satisfies `sum xᵢ - 1 ≤ 0` but is not OneHot. -/
theorem oneHot_lessEqual_is_not_equivalent :
    let state : State 2 := fun _ => 0
    wrongSenseOneHotSource.Holds state ∧
      ¬({ members := allTwo } : OneHotConstraint 2).Holds state := by
  native_decide

def scaledSOS1Source : LinearConstraint 2 :=
  { expr := Affine.scale 3 (oneHotExpr allTwo), sense := .lessEqual }

def sos1Draft : BinaryCardinalitySOS1Draft 2 :=
  { members := allTwo, scale := 3 }

example : checkBinaryCardinalitySOS1 binaryDomains2 scaledSOS1Source sos1Draft = true := by
  native_decide

def negativeSOS1Draft : BinaryCardinalitySOS1Draft 2 :=
  { members := allTwo, scale := -3 }

example : checkBinaryCardinalitySOS1 binaryDomains2
    { expr := Affine.scale (-3) (oneHotExpr allTwo), sense := .lessEqual }
    negativeSOS1Draft = false := by
  native_decide

def wrongSenseSOS1Source : LinearConstraint 2 :=
  { expr := Affine.scale 3 (oneHotExpr allTwo), sense := .equal }

example : checkBinaryCardinalitySOS1 binaryDomains2 wrongSenseSOS1Source
    sos1Draft = false := by native_decide

/-- An equality with the cardinality affine expression excludes the all-zero
state, which is valid SOS1. Thus checking the row sense is essential. -/
theorem sos1_equal_is_not_equivalent :
    let state : State 2 := fun _ => 0
    ¬wrongSenseSOS1Source.Holds state ∧
      ({ members := allTwo } : SOS1Constraint 2).Holds state := by
  dsimp
  constructor
  · simp only [wrongSenseSOS1Source, LinearConstraint.Holds]
    rw [Affine.eval_scale, eval_oneHotExpr]
    norm_num [allTwo]
  · simp [SOS1Constraint.Holds]

def twoVarAffine (xCoefficient zCoefficient constant : Rat) : Affine 2 where
  coeff := fun i => if i.val = 0 then xCoefficient else zCoefficient
  constant := constant

def selectorPrivateExample : Finset (Fin 2) := {1}

def selectorIsolationDomains : Fin 2 → Domain := fun i =>
  if i.val = 0 then
    .continuous (.finite (-1) 1 (by norm_num))
  else
    .continuous

def selectorIsolationBase : Instance 2 where
  domains := selectorIsolationDomains
  constraints := []
  objective := twoVarAffine 1 0 0
  sense := .minimize

def selectorIsolationWitness : Instance.SelectorIsolationWitness 2 where
  privateSelectors := selectorPrivateExample

example : selectorIsolationBase.checkSelectorIsolation
    selectorIsolationWitness = true := by native_decide

/-- The same witness is rejected as soon as the base objective observes the
claimed private selector coordinate. -/
def selectorLeakingBase : Instance 2 :=
  { selectorIsolationBase with objective := twoVarAffine 1 1 0 }

example : selectorLeakingBase.checkSelectorIsolation
    selectorIsolationWitness = false := by native_decide

/-- Without selector isolation, changing only the private variable can change
the objective, so it cannot be removed soundly. -/
theorem selector_leak_changes_objective :
    let lhs : State 2 := fun _ => 0
    let rhs : State 2 := fun i => if i.val = 0 then 0 else 1
    AgreeOutside selectorPrivateExample lhs rhs ∧
      selectorLeakingBase.ObjectiveValue lhs ≠
        selectorLeakingBase.ObjectiveValue rhs := by
  dsimp
  constructor
  · intro i houtside
    fin_cases i
    · rfl
    · exact False.elim (houtside (by simp [selectorPrivateExample]))
  · norm_num [selectorLeakingBase, selectorIsolationBase,
      Instance.ObjectiveValue, twoVarAffine, Affine.eval]

/-! Mixed SDK selector layout: member 0 is reused as its own binary selector,
while member 1 gets a fresh selector. Its lower bound is zero, so the lower
link is omitted exactly as in `Instance::convert_sos1_to_constraints`. -/

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

def invalidPlannedMembersExample : Fin 2 → Rat := fun i => if i.val = 0 then 1 else 2

example : ¬PlannedSelectorFormulation plannedReusedExample plannedBoundsExample
    invalidPlannedMembersExample plannedFreshSelectorsExample := by
  native_decide

end OMMXProof.Test.Fixtures
