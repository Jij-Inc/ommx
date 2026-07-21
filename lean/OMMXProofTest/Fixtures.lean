import OMMXProof.Linear.Farkas
import OMMXProof.Reduction
import OMMXProof.Special.OneHot
import OMMXProof.Special.Indicator
import OMMXProof.Special.SOS1

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
    (state : state 1) :
    (LinearConstraint.normalize lhs rhs sense).Holds state ↔
      match sense with
      | .lessEqual => lhs.eval state ≤ rhs.eval state
      | .equal => lhs.eval state = rhs.eval state :=
  LinearConstraint.normalize_holds_iff lhs rhs sense state

def upperOne : Affine 1 := oneVarAffine 1 (-1)
def lowerZero : Affine 1 := oneVarAffine (-1) 0

def bounded : LinearSystem 1 where
  ineqCount := 2
  eqCount := 0
  inequalities := fun i => if i.val = 0 then upperOne else lowerZero
  equalities := fun i => nomatch i

def twiceUpper : Affine 1 := oneVarAffine 2 (-2)

def twiceUpperWitness : FarkasWitness bounded where
  inequalityWeights := fun i => if i.val = 0 then 2 else 0
  equalityWeights := fun i => nomatch i

example : twiceUpperWitness.checkImplication twiceUpper = true := by native_decide

/-- Scalar slack is accepted: `2x ≤ 2` implies the weaker `2x ≤ 3`. -/
def weakerTarget : Affine 1 := oneVarAffine 2 (-3)

example : twiceUpperWitness.checkImplication weakerTarget = true := by native_decide

def tooStrongTarget : Affine 1 := oneVarAffine 2 (-1)

example : twiceUpperWitness.checkImplication tooStrongTarget = false := by native_decide

def negativeWeightWitness : FarkasWitness bounded where
  inequalityWeights := fun i => if i.val = 0 then -1 else 0
  equalityWeights := fun i => nomatch i

example : negativeWeightWitness.checkImplication (oneVarAffine (-1) 1) = false := by
  native_decide

def fixedAtZero : LinearSystem 1 where
  ineqCount := 0
  eqCount := 1
  inequalities := fun i => nomatch i
  equalities := fun _ => oneVarAffine 1 0

def freeEqualityWeight : FarkasWitness fixedAtZero where
  inequalityWeights := fun i => nomatch i
  equalityWeights := fun _ => -1

example : freeEqualityWeight.checkImplication (oneVarAffine (-1) 0) = true := by
  native_decide

def fixedEqualityWitness : ImpliedEqualityWitness fixedAtZero where
  upper :=
    { inequalityWeights := fun i => nomatch i
      equalityWeights := fun _ => 1 }
  lower := freeEqualityWeight

example : fixedEqualityWitness.check (oneVarAffine 1 0) = true := by
  native_decide

def invalidFixedEqualityWitness : ImpliedEqualityWitness fixedAtZero where
  upper := fixedEqualityWitness.upper
  lower := fixedEqualityWitness.upper

example : invalidFixedEqualityWitness.check (oneVarAffine 1 0) = false := by
  native_decide

def impossible : LinearSystem 1 where
  ineqCount := 2
  eqCount := 0
  inequalities := fun i =>
    if i.val = 0 then oneVarAffine 1 0 else oneVarAffine (-1) 1
  equalities := fun i => nomatch i

def impossibleWitness : FarkasWitness impossible where
  inequalityWeights := fun _ => 1
  equalityWeights := fun i => nomatch i

example : impossibleWitness.checkInfeasibility = true := by native_decide

def invalidImpossibleWitness : FarkasWitness impossible where
  inequalityWeights := fun i => if i.val = 0 then 1 else 0
  equalityWeights := fun i => nomatch i

example : invalidImpossibleWitness.checkInfeasibility = false := by native_decide

theorem impossible_has_no_solution : ¬ ∃ state, impossible.Feasible state :=
  FarkasWitness.checkInfeasibility_sound (witness := impossibleWitness)
    (by native_decide)

def oneVariableDomains : Fin 1 → VariableDomain := fun _ =>
  { kind := .continuous, bounds := { lower := some 0, upper := some 1 } }

def storedBounds : Fin 2 → BoundSide 1 := fun i =>
  if i.val = 0 then .upper 0 1 else .lower 0 0

def activityUpperWitness : ActivityBoundWitness storedBounds where
  inequalityWeights := fun i => if i.val = 0 then 1 else 0
  equalityWeights := fun i => nomatch i

example : checkActivityBoundForDomains oneVariableDomains storedBounds
    activityUpperWitness upperOne = true := by native_decide

def inventedBounds : Fin 1 → BoundSide 1 := fun _ => .upper 0 2

def inventedBoundWitness : ActivityBoundWitness inventedBounds where
  inequalityWeights := fun _ => 1
  equalityWeights := fun i => nomatch i

example : checkActivityBoundForDomains oneVariableDomains inventedBounds
    inventedBoundWitness (oneVarAffine 1 (-2)) = false := by native_decide

def tighteningRemaining : LinearSystem 1 where
  ineqCount := 1
  eqCount := 0
  inequalities := fun _ => oneVarAffine 1 0
  equalities := fun i => nomatch i

def storedUpperOne : BoundSide 1 := .upper 0 1
def tightenedUpperZero : BoundSide 1 := .upper 0 0
def weakenedUpperTwo : BoundSide 1 := .upper 0 2

def tighteningWitness : BoundTighteningWitness tighteningRemaining storedUpperOne where
  inequalityWeights := fun i => if i.val = 0 then 0 else 1
  equalityWeights := fun i => nomatch i

example : checkBoundTightening oneVariableDomains tighteningRemaining
    storedUpperOne tightenedUpperZero tighteningWitness = true := by
  native_decide

def weakeningWitness : BoundTighteningWitness tighteningRemaining storedUpperOne where
  inequalityWeights := fun i => if i.val = 0 then 1 else 0
  equalityWeights := fun i => nomatch i

/-- Even a valid implication cannot authorize replacing a stored bound by a
weaker side. -/
example : checkBoundTightening oneVariableDomains tighteningRemaining
    storedUpperOne weakenedUpperTwo weakeningWitness = false := by
  native_decide

/-- The target row cannot occur in its own witness system. -/
def remainingWithoutTarget : LinearSystem 1 where
  ineqCount := 1
  eqCount := 0
  inequalities := fun _ => upperOne
  equalities := fun i => nomatch i

def redundantWitness : FarkasWitness remainingWithoutTarget where
  inequalityWeights := fun _ => 2
  equalityWeights := fun i => nomatch i

example : redundantWitness.checkImplication twiceUpper = true := by native_decide

theorem redundant_removal_preserves (state : state 1) :
    RowExtensionFeasible remainingWithoutTarget twiceUpper state ↔
      remainingWithoutTarget.Feasible state :=
  redundantRow_iff (witness := redundantWitness) (by native_decide) state

def emptySystem : LinearSystem 1 where
  ineqCount := 0
  eqCount := 0
  inequalities := fun i => nomatch i
  equalities := fun i => nomatch i

def emptyWitness : FarkasWitness emptySystem where
  inequalityWeights := fun i => nomatch i
  equalityWeights := fun i => nomatch i

/-- Integrality cannot be smuggled into the continuous Farkas kernel. -/
example : emptyWitness.checkImplication upperOne = false := by native_decide

def binaryDomain : VariableDomain :=
  { kind := .binary, bounds := { lower := some 0, upper := some 1 } }

def binaryDomains2 : Fin 2 → VariableDomain := fun _ => binaryDomain

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

def continuousDomains2 : Fin 2 → VariableDomain := fun _ =>
  { kind := .continuous }

example : checkOneHot continuousDomains2 scaledOneHotSource oneHotDraft = false := by
  native_decide

/-- The binary-domain premise is essential: `(2, -1)` satisfies the structural
sum equation but is not a OneHot state. -/
theorem continuous_sum_is_not_oneHot :
    let state : state 2 := fun i => if i.val = 0 then 2 else -1
    (oneHotExpr allTwo).eval state = 0 ∧
      ¬(SpecialConstraint.oneHot allTwo).Holds state := by
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
    let state : state 2 := fun _ => 0
    wrongSenseOneHotSource.Holds state ∧
      ¬(SpecialConstraint.oneHot allTwo).Holds state := by
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
    let state : state 2 := fun _ => 0
    ¬wrongSenseSOS1Source.Holds state ∧
      (SpecialConstraint.sos1 allTwo).Holds state := by
  dsimp
  constructor
  · simp only [wrongSenseSOS1Source, LinearConstraint.Holds]
    rw [Affine.eval_scale, eval_oneHotExpr]
    norm_num [allTwo]
  · simp [SpecialConstraint.Holds]

def twoVarAffine (xCoefficient zCoefficient constant : Rat) : Affine 2 where
  coeff := fun i => if i.val = 0 then xCoefficient else zCoefficient
  constant := constant

/-- Surviving row `x ≤ 0`. -/
def indicatorSurviving : LinearSystem 2 where
  ineqCount := 1
  eqCount := 0
  inequalities := fun _ => twoVarAffine 1 0 0
  equalities := fun i => nomatch i

/-- Source Big-M row `x - 10z ≤ 0`. -/
def indicatorSource : LinearConstraint 2 :=
  { expr := twoVarAffine 1 (-10) 0, sense := .lessEqual }

/-- Active-on-one body after exact substitution: `x - 10 ≤ 0`. -/
def indicatorBody : LinearConstraint 2 :=
  { expr := twoVarAffine 1 0 (-10), sense := .lessEqual }

def indicatorDomains : Fin 2 → VariableDomain := fun i =>
  if i.val = 0 then { kind := .continuous } else binaryDomain

def indicatorWitness :
    IndicatorReplaceWitness indicatorSurviving 1 .activeOnOne where
  inactive :=
    { inequalityWeights := fun _ => 1
      equalityWeights := fun _ => -10 }

example : checkIndicatorReplace indicatorDomains indicatorSurviving
    indicatorSource indicatorBody 1 .activeOnOne indicatorWitness = true := by
  native_decide

def wrongIndicatorBody : LinearConstraint 2 :=
  { expr := twoVarAffine 1 0 (-9), sense := .lessEqual }

example : checkIndicatorReplace indicatorDomains indicatorSurviving
    indicatorSource wrongIndicatorBody 1 .activeOnOne indicatorWitness = false := by
  native_decide

def wrongSenseIndicatorSource : LinearConstraint 2 :=
  { expr := indicatorSource.expr, sense := .equal }

def wrongSenseIndicatorBody : LinearConstraint 2 :=
  { expr := indicatorBody.expr, sense := .equal }

example : checkIndicatorReplace indicatorDomains indicatorSurviving
    wrongSenseIndicatorSource wrongSenseIndicatorBody 1 .activeOnOne
      indicatorWitness = false := by
  native_decide

/-- Treating an equality source as an inequality replacement would be unsound:
on the inactive branch the Indicator is vacuous while the source equality may
still fail, even under the surviving row. -/
theorem indicator_wrong_sense_is_not_replacement :
    let state : state 2 := fun i => if i.val = 0 then -1 else 0
    indicatorSurviving.Feasible state ∧
      ¬wrongSenseIndicatorSource.Holds state ∧
      (SpecialConstraint.indicator 1 .activeOnOne
        wrongSenseIndicatorBody).Holds state := by
  native_decide

/-- Equality source rows need both inactive directions from the same surviving
system. Here the surviving equality is `x = 0`. -/
def indicatorEqualitySurviving : LinearSystem 2 where
  ineqCount := 0
  eqCount := 1
  inequalities := fun i => nomatch i
  equalities := fun _ => twoVarAffine 1 0 0

def indicatorEqualitySource : LinearConstraint 2 :=
  { expr := twoVarAffine 1 (-10) 0, sense := .equal }

def indicatorEqualityBody : LinearConstraint 2 :=
  { expr := twoVarAffine 1 0 (-10), sense := .equal }

def indicatorEqualityWitness :
    EqualityIndicatorReplaceWitness indicatorEqualitySurviving 1 .activeOnOne where
  upper :=
    { inequalityWeights := fun i => nomatch i
      equalityWeights := fun i => if i.val = 0 then -10 else 1 }
  lower :=
    { inequalityWeights := fun i => nomatch i
      equalityWeights := fun i => if i.val = 0 then 10 else -1 }

example : checkEqualityIndicatorReplace indicatorDomains indicatorEqualitySurviving
    indicatorEqualitySource indicatorEqualityBody 1 .activeOnOne
      indicatorEqualityWitness = true := by
  native_decide

def oneSidedEqualityWitness :
    EqualityIndicatorReplaceWitness indicatorEqualitySurviving 1 .activeOnOne where
  upper := indicatorEqualityWitness.upper
  lower := indicatorEqualityWitness.upper

example : checkEqualityIndicatorReplace indicatorDomains indicatorEqualitySurviving
    indicatorEqualitySource indicatorEqualityBody 1 .activeOnOne
      oneSidedEqualityWitness = false := by
  native_decide

/-! The forward SDK algorithm may omit the lower equality side when the exact
lower bound is zero.  The remaining upper side plus the base bound still
preserves the Indicator semantics. -/

def sdkIndicatorBase (state : state 2) : Prop :=
  VariableDomain.KindHolds .binary (state 1) ∧
    0 ≤ state 0 ∧ state 0 ≤ 3

def sdkIndicatorBody (state : state 2) : Rat := state 0

def sdkIndicatorObjective (state : state 2) : Rat := state 0

def sdkIndicatorEqualityPreserves :
    IdentityPreserves
      (replaceProblem sdkIndicatorBase
        (fun state =>
          IndicatorBigM.UpperSide sdkIndicatorBody 1 3 state ∧
            IndicatorBigM.LowerSide sdkIndicatorBody 1 0 state)
        sdkIndicatorObjective .minimize)
      (replaceProblem sdkIndicatorBase
        (IndicatorPredicate 1 .activeOnOne
          (fun state => sdkIndicatorBody state = 0))
        sdkIndicatorObjective .minimize) :=
  IndicatorBigM.equality_preserves sdkIndicatorBase sdkIndicatorBody 1 0 3
    sdkIndicatorObjective .minimize
    (by intro state hbase; exact hbase.1)
    (by intro state hbase; exact ⟨hbase.2.1, hbase.2.2⟩)

example (state : state 2) :
    (replaceProblem sdkIndicatorBase
      (fun x =>
        IndicatorBigM.UpperSide sdkIndicatorBody 1 3 x ∧
          IndicatorBigM.LowerSide sdkIndicatorBody 1 0 x)
      sdkIndicatorObjective .minimize).feasible state ↔
    (replaceProblem sdkIndicatorBase
      (IndicatorPredicate 1 .activeOnOne
        (fun x => sdkIndicatorBody x = 0))
      sdkIndicatorObjective .minimize).feasible state :=
  sdkIndicatorEqualityPreserves.feasible_iff state

def selectorBoundsExample : SelectorBounds (Fin 1) :=
  ⟨fun _ => -1, fun _ => 1⟩

def selectorBaseExample (members : Fin 1 → Rat) : Prop :=
  WithinSelectorBounds selectorBoundsExample members

def selectorObjectiveExample (members : Fin 1 → Rat) : Rat := members 0

def selectorPrivateExample : Finset (Fin 2) := {1}

def selectorIsolationDomains : Fin 2 → VariableDomain := fun i =>
  if i.val = 0 then
    { kind := .continuous, bounds := { lower := some (-1), upper := some 1 } }
  else
    { kind := .continuous }

def selectorIsolationLinear : LinearSystem 2 where
  ineqCount := 0
  eqCount := 0
  inequalities := fun i => nomatch i
  equalities := fun i => nomatch i

def selectorIsolationBase : CoreModel 2 where
  domains := selectorIsolationDomains
  linear := selectorIsolationLinear
  objective := twoVarAffine 1 0 0
  sense := .minimize

def selectorIsolationWitness : CoreModel.SelectorIsolationWitness 2 where
  privateSelectors := selectorPrivateExample

example : selectorIsolationBase.checkSelectorIsolation
    selectorIsolationWitness = true := by native_decide

/-- The same witness is rejected as soon as the base objective observes the
claimed private selector coordinate. -/
def selectorLeakingBase : CoreModel 2 :=
  { selectorIsolationBase with objective := twoVarAffine 1 1 0 }

example : selectorLeakingBase.checkSelectorIsolation
    selectorIsolationWitness = false := by native_decide

/-- Without selector isolation, changing only the private coordinate can change
the objective, so compression would not preserve objective values. -/
theorem selector_leak_changes_objective :
    let lhs : state 2 := fun _ => 0
    let rhs : state 2 := fun i => if i.val = 0 then 0 else 1
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
      CoreModel.ObjectiveValue, twoVarAffine, Affine.eval]

def selectorPairEncoding
    (pair : (Fin 1 → Rat) × (Fin 1 → Rat)) : state 2 :=
  fun i => if i.val = 0 then pair.1 0 else pair.2 0

theorem selectorPairEncoding_respectsIsolation :
    EncodingRespectsIsolation selectorPairEncoding selectorIsolationWitness := by
  intro members selectors selectors' index houtside
  fin_cases index
  · simp [selectorPairEncoding]
  · simp [selectorIsolationWitness, selectorPrivateExample] at houtside

theorem selectorIsolationBase_bounds {members : Fin 1 → Rat}
    (hfeasible : selectorIsolationBase.Feasible
      (selectorPairEncoding (members, zeroSelectors))) :
    WithinSelectorBounds selectorBoundsExample members := by
  intro i
  fin_cases i
  have hdomain := hfeasible.1 (0 : Fin 2)
  simpa [selectorIsolationBase, selectorIsolationDomains,
    selectorPairEncoding, selectorBoundsExample, VariableDomain.Holds,
    VariableDomain.KindHolds, Bounds.Holds] using hdomain

/-- The executable isolation check is consumed directly by the exact
`ProjectionPreserves` compression theorem. -/
def checkedSelectorCompressionExample :
    ProjectionPreserves
      (coreSelectorSourceProblem selectorIsolationBase selectorPairEncoding
        selectorBoundsExample)
      (coreSOS1TargetProblem selectorIsolationBase selectorPairEncoding) :=
  coreSelectorCompression selectorIsolationBase selectorPairEncoding
    selectorBoundsExample selectorIsolationWitness (by native_decide)
    selectorPairEncoding_respectsIsolation selectorIsolationBase_bounds

def selectorCompressionExample :
    ProjectionPreserves
      (selectorSourceProblem selectorBoundsExample selectorBaseExample
        selectorObjectiveExample .minimize)
      (sos1TargetProblem selectorBaseExample selectorObjectiveExample .minimize) :=
  selectorCompression selectorBoundsExample selectorBaseExample
    selectorObjectiveExample .minimize (by intro members h; exact h)

/-! Mixed SDK plan: member 0 is reused as its own binary selector, while
member 1 gets a fresh selector.  Its lower bound is zero, so the lower link is
omitted exactly as in `Instance::convert_sos1_to_constraints`. -/

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

def plannedBaseExample (members : Fin 2 → Rat) : Prop :=
  WithinSelectorBounds plannedBoundsExample members ∧
    GenericBinaryOn plannedReusedExample members

def plannedObjectiveExample (members : Fin 2 → Rat) : Rat :=
  members 0 + members 1

def plannedMembersExample : Fin 2 → Rat := fun i => if i.val = 0 then 0 else 2

def plannedFreshSelectorsExample : Fin 2 → Rat := fun _ => 1

example : PlannedSelectorGadget plannedReusedExample plannedBoundsExample
    plannedMembersExample plannedFreshSelectorsExample := by
  native_decide

def invalidPlannedMembersExample : Fin 2 → Rat := fun i => if i.val = 0 then 1 else 2

example : ¬PlannedSelectorGadget plannedReusedExample plannedBoundsExample
    invalidPlannedMembersExample plannedFreshSelectorsExample := by
  native_decide

def plannedSelectorCompressionExample :
    ProjectionPreserves
      (plannedSelectorSourceProblem plannedReusedExample plannedBoundsExample
        plannedBaseExample plannedObjectiveExample .minimize)
      (sos1TargetProblem plannedBaseExample plannedObjectiveExample .minimize) :=
  plannedSelectorCompression plannedReusedExample plannedBoundsExample
    plannedBaseExample plannedObjectiveExample .minimize
    { freshBoundsContainZero := by native_decide
      baseBounds := by intro members hbase; exact hbase.1
      baseReusedBinary := by intro members hbase; exact hbase.2 }

def checkedPlannedSelectorCompressionExample :
    ProjectionPreserves
      (corePlannedSelectorSourceProblem selectorIsolationBase selectorPairEncoding
        ∅ selectorBoundsExample)
      (coreSOS1TargetProblem selectorIsolationBase selectorPairEncoding) :=
  corePlannedSelectorCompression selectorIsolationBase selectorPairEncoding
    ∅ selectorBoundsExample selectorIsolationWitness (by native_decide)
    selectorPairEncoding_respectsIsolation
    { freshBoundsContainZero := by native_decide
      baseBounds := selectorIsolationBase_bounds
      baseReusedBinary := by intro members _ i hi; simp at hi }

end OMMXProof.Test.Fixtures
